use std::sync::Arc;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

use crate::application::use_cases::ManageSessionUseCase;
use crate::domain::{
    LeadRepository, NewWatchTarget, OutboxRepository, PostRepository, SessionRepository, TargetRepository,
};
use crate::infrastructure::SqliteDatabase;

#[derive(Clone)]
pub struct AppState {
    pub db: SqliteDatabase,
    pub target_repo: Arc<dyn TargetRepository>,
    pub post_repo: Arc<dyn PostRepository>,
    pub lead_repo: Arc<dyn LeadRepository>,
    pub outbox_repo: Arc<dyn OutboxRepository>,
    pub session_repo: Arc<dyn SessionRepository>,
    pub session_use_case: Arc<ManageSessionUseCase>,
}

#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTargetRequest {
    pub platform: Option<String>,
    pub group_id: String,
    pub display_name: String,
    pub session_id: Option<String>,
    pub enabled: Option<bool>,
    pub interval_seconds: Option<i32>,
    pub ordering: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ToggleTargetRequest {
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct RunsParams {
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct PostsQueryParams {
    pub platform: Option<String>,
    #[serde(alias = "groupId")]
    pub group_id: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// ── Handlers: Targets ────────────────────────────────────────────────────────

pub async fn list_targets(State(state): State<AppState>) -> impl IntoResponse {
    match state.target_repo.get_all_targets().await {
        Ok(targets) => (StatusCode::OK, Json(serde_json::json!({ "targets": targets }))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))),
    }
}

pub async fn create_target(
    State(state): State<AppState>,
    Json(payload): Json<CreateTargetRequest>,
) -> impl IntoResponse {
    let new_target = NewWatchTarget {
        platform: payload.platform.unwrap_or_else(|| "facebook".to_string()),
        group_id: payload.group_id,
        display_name: payload.display_name,
        session_id: payload.session_id.unwrap_or_else(|| "primary".to_string()),
        enabled: payload.enabled.unwrap_or(true),
        interval_seconds: payload.interval_seconds.unwrap_or(300),
        ordering: payload.ordering.unwrap_or_else(|| "chronological".to_string()),
    };

    match state.target_repo.create_target(new_target).await {
        Ok(target) => (StatusCode::CREATED, Json(serde_json::json!({ "target": target }))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))),
    }
}

pub async fn toggle_target(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    payload: Option<Json<ToggleTargetRequest>>,
) -> impl IntoResponse {
    let target = match state.target_repo.get_target_by_id(id).await {
        Ok(Some(t)) => t,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "Target not found" }))),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))),
    };

    let new_state = match payload {
        Some(Json(req)) => req.enabled.unwrap_or(!target.enabled),
        None => !target.enabled,
    };

    match state.target_repo.toggle_target(id, new_state).await {
        Ok(true) => (StatusCode::OK, Json(serde_json::json!({ "success": true, "id": id, "enabled": new_state }))),
        Ok(false) => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "Target not found" }))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))),
    }
}

// ── Handlers: Session Cookie Sync & Status ───────────────────────────────────

pub async fn get_session_status(State(state): State<AppState>) -> impl IntoResponse {
    match state.session_repo.get_session("primary").await {
        Ok(Some(s)) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "id": s.id,
                "platform": s.platform,
                "status": s.status,
                "account_alias": s.account_alias,
                "accountAlias": s.account_alias,
                "version": s.session_version,
                "last_verified_at": s.last_verified_at,
                "lastVerifiedAt": s.last_verified_at,
                "message": format!("Session is {}", s.status)
            })),
        ),
        Ok(None) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "id": "primary",
                "platform": "facebook",
                "status": "UNAVAILABLE",
                "account_alias": "None",
                "version": 0,
                "message": "Session not initialized"
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

pub async fn import_session_cookies(
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let mut session_id = "primary".to_string();
    let mut cookies_array: Vec<serde_json::Value> = Vec::new();

    if let Some(arr) = payload.as_array() {
        cookies_array = arr.clone();
    } else if let Some(obj) = payload.as_object() {
        if let Some(sid) = obj.get("session_id").and_then(|v| v.as_str()) {
            session_id = sid.to_string();
        }
        if let Some(cookies) = obj.get("cookies") {
            if let Some(arr) = cookies.as_array() {
                cookies_array = arr.clone();
            } else if let Some(s) = cookies.as_str() {
                if let Ok(parsed_arr) = serde_json::from_str::<Vec<serde_json::Value>>(s) {
                    cookies_array = parsed_arr;
                }
            }
        } else if obj.contains_key("c_user") {
            for (k, v) in obj {
                cookies_array.push(serde_json::json!({
                    "name": k,
                    "value": v.as_str().unwrap_or(""),
                    "domain": ".facebook.com",
                    "path": "/"
                }));
            }
        }
    }

    if cookies_array.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "success": false, "error": "Không tìm thấy danh sách cookies hợp lệ trong payload" })),
        );
    }

    // Find c_user
    let mut user_id = String::new();
    for c in &cookies_array {
        if let Some(name) = c.get("name").and_then(|n| n.as_str()) {
            if name == "c_user" {
                if let Some(val) = c.get("value").and_then(|v| v.as_str()) {
                    user_id = val.to_string();
                }
            }
        }
    }

    let cookies_count = cookies_array.len();
    let cookies_json = match serde_json::to_string(&cookies_array) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "success": false, "error": e.to_string() })),
            );
        }
    };

    match state.session_use_case.import_cookies(&session_id, &cookies_json).await {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "success": true,
                "status": "READY",
                "session_id": session_id,
                "userId": user_id,
                "user_id": user_id,
                "cookiesCount": cookies_count,
                "cookies_count": cookies_count,
                "message": "Nạp session cookies thành công"
            })),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "success": false, "error": e.to_string() })),
        ),
    }
}

// ── Handlers: System Status & Runs ──────────────────────────────────────────

pub async fn get_system_status(State(state): State<AppState>) -> impl IntoResponse {
    let targets = match state.target_repo.get_all_targets().await {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))),
    };
    let targets_total = targets.len();
    let targets_enabled = targets.iter().filter(|t| t.enabled).count();

    let primary_session = state.session_repo.get_session("primary").await.unwrap_or(None);
    let sessions = match primary_session {
        Some(s) => vec![serde_json::json!({
            "id": s.id,
            "platform": s.platform,
            "status": s.status,
            "account_alias": s.account_alias,
            "version": s.session_version,
            "last_verified_at": s.last_verified_at
        })],
        None => vec![],
    };

    let pool = state.db.pool();
    let posts_total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM social_posts")
        .fetch_one(pool)
        .await
        .unwrap_or((0,));
    let leads_total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM social_leads")
        .fetch_one(pool)
        .await
        .unwrap_or((0,));
    let pending_jobs: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM social_jobs WHERE status = 'PENDING'")
        .fetch_one(pool)
        .await
        .unwrap_or((0,));
    let pending_outbox: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM social_outbox WHERE status = 'PENDING'")
        .fetch_one(pool)
        .await
        .unwrap_or((0,));
    let dead_letter_outbox: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM social_outbox WHERE status = 'DEAD_LETTER'")
        .fetch_one(pool)
        .await
        .unwrap_or((0,));

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "service": "social-listener-rust",
            "env": "production",
            "sessions": sessions,
            "targets": {
                "total": targets_total,
                "enabled": targets_enabled
            },
            "posts_total": posts_total.0,
            "leads_total": leads_total.0,
            "queues": {
                "pending_classification_jobs": pending_jobs.0,
                "pending_outbox_events": pending_outbox.0,
                "dead_letter_outbox_events": dead_letter_outbox.0
            }
        })),
    )
}

pub async fn list_runs(
    State(state): State<AppState>,
    Query(params): Query<RunsParams>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(20).clamp(1, 100);
    let pool = state.db.pool();

    #[derive(sqlx::FromRow, serde::Serialize)]
    struct RunRow {
        run_id: String,
        target_id: i64,
        started_at: chrono::DateTime<chrono::Utc>,
        finished_at: Option<chrono::DateTime<chrono::Utc>>,
        status: String,
        observed_count: i32,
        new_count: i32,
        stop_reason: Option<String>,
        error_code: Option<String>,
        target: Option<String>,
        platform: Option<String>,
    }

    let runs = sqlx::query_as::<_, RunRow>(
        r#"
        SELECT 
            r.run_id,
            r.target_id,
            r.started_at,
            r.finished_at,
            r.status,
            r.observed_count,
            r.new_count,
            r.stop_reason,
            r.error_code,
            COALESCE(t.display_name, CAST(r.target_id AS TEXT)) as target,
            COALESCE(t.platform, 'facebook') as platform
        FROM social_crawl_runs r
        LEFT JOIN social_watch_targets t ON r.target_id = t.id
        ORDER BY r.started_at DESC
        LIMIT ?
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await;

    match runs {
        Ok(r) => (StatusCode::OK, Json(serde_json::json!({ "runs": r }))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))),
    }
}

// ── Handlers: Posts ──────────────────────────────────────────────────────────

pub async fn list_posts(
    State(state): State<AppState>,
    Query(params): Query<PostsQueryParams>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(50).clamp(1, 200);
    let offset = params.offset.unwrap_or(0).max(0);
    let pool = state.db.pool();

    #[derive(sqlx::FromRow)]
    struct PostRow {
        id: i64,
        platform: String,
        group_id: String,
        post_id: String,
        canonical_url: String,
        author_display: String,
        text: String,
        source_created_at: Option<chrono::DateTime<chrono::Utc>>,
        first_seen_at: chrono::DateTime<chrono::Utc>,
        is_lead: bool,
        lead_intent: Option<String>,
        lead_confidence: Option<f64>,
        lead_location: Option<String>,
        target_name: Option<String>,
    }

    let query = r#"
        SELECT 
            p.id,
            p.platform,
            p.group_id,
            p.post_id,
            p.canonical_url,
            p.author_display,
            p.text,
            p.source_created_at,
            p.first_seen_at,
            (l.id IS NOT NULL) as is_lead,
            l.intent as lead_intent,
            l.confidence as lead_confidence,
            l.location as lead_location,
            t.display_name as target_name
        FROM social_posts p
        LEFT JOIN social_leads l ON p.id = l.post_id
        LEFT JOIN social_watch_targets t ON p.group_id = t.group_id AND p.platform = t.platform
        WHERE (? IS NULL OR p.platform = ?)
          AND (? IS NULL OR p.group_id = ?)
        ORDER BY p.id DESC
        LIMIT ? OFFSET ?
    "#;

    let posts = sqlx::query_as::<_, PostRow>(query)
        .bind(params.platform.as_deref())
        .bind(params.platform.as_deref())
        .bind(params.group_id.as_deref())
        .bind(params.group_id.as_deref())
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await;

    match posts {
        Ok(rows) => {
            let items: Vec<serde_json::Value> = rows
                .into_iter()
                .map(|p| {
                    let excerpt = if p.text.chars().count() > 160 {
                        format!("{}...", p.text.chars().take(160).collect::<String>())
                    } else {
                        p.text.clone()
                    };
                    serde_json::json!({
                        "id": p.id,
                        "platform": p.platform,
                        "group_id": p.group_id,
                        "post_id": p.post_id,
                        "canonical_url": p.canonical_url,
                        "author": p.author_display,
                        "author_display": p.author_display,
                        "text": p.text,
                        "excerpt": excerpt,
                        "source_created_at": p.source_created_at,
                        "first_seen_at": p.first_seen_at,
                        "is_lead": p.is_lead,
                        "lead_intent": p.lead_intent,
                        "lead_confidence": p.lead_confidence,
                        "lead_location": p.lead_location,
                        "target_name": p.target_name
                    })
                })
                .collect();

            (StatusCode::OK, Json(serde_json::json!({ "posts": items, "limit": limit, "offset": offset })))
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))),
    }
}

// ── Handlers: Leads & Outbox ─────────────────────────────────────────────────

pub async fn list_leads(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(50).clamp(1, 200);
    let offset = params.offset.unwrap_or(0).max(0);

    match state.lead_repo.list_leads(limit, offset).await {
        Ok(leads) => (StatusCode::OK, Json(serde_json::json!({ "leads": leads, "limit": limit, "offset": offset }))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))),
    }
}

pub async fn list_outbox(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(50).clamp(1, 200);
    let offset = params.offset.unwrap_or(0).max(0);

    match state.outbox_repo.list_events(limit, offset).await {
        Ok(events) => (StatusCode::OK, Json(serde_json::json!({ "events": events, "limit": limit, "offset": offset }))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))),
    }
}

pub async fn retry_outbox_event(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.outbox_repo.retry_dead_letter(id).await {
        Ok(true) => (StatusCode::OK, Json(serde_json::json!({ "success": true, "id": id }))),
        Ok(false) => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "Event not found" }))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))),
    }
}

// ── Health Probes & Metrics ──────────────────────────────────────────────────

pub async fn health_liveness() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({ "status": "UP", "version": "v3.0.0" })))
}

pub async fn health_readiness(State(state): State<AppState>) -> impl IntoResponse {
    match state.target_repo.get_all_targets().await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({ "status": "READY", "database": "CONNECTED" }))),
        Err(e) => (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({ "status": "NOT_READY", "error": e.to_string() }))),
    }
}

pub async fn health_freshness() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({ "status": "FRESH" })))
}

pub async fn metrics() -> impl IntoResponse {
    let body = format!(
        "# HELP social_listener_up Liveness indicator\n# TYPE social_listener_up gauge\nsocial_listener_up 1\n"
    );
    (StatusCode::OK, [("content-type", "text/plain; version=0.0.4")], body)
}
