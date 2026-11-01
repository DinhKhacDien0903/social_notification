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

#[derive(Clone)]
pub struct AppState {
    pub target_repo: Arc<dyn TargetRepository>,
    pub post_repo: Arc<dyn PostRepository>,
    pub lead_repo: Arc<dyn LeadRepository>,
    pub outbox_repo: Arc<dyn OutboxRepository>,
    #[allow(dead_code)]
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
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct ImportCookieRequest {
    pub session_id: Option<String>,
    pub cookies: String,
}

// Handlers: Targets
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
    Json(payload): Json<ToggleTargetRequest>,
) -> impl IntoResponse {
    match state.target_repo.toggle_target(id, payload.enabled).await {
        Ok(true) => (StatusCode::OK, Json(serde_json::json!({ "success": true, "id": id, "enabled": payload.enabled }))),
        Ok(false) => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "Target not found" }))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))),
    }
}

// Handlers: Session Cookie Sync
pub async fn import_session_cookies(
    State(state): State<AppState>,
    Json(payload): Json<ImportCookieRequest>,
) -> impl IntoResponse {
    let session_id = payload.session_id.unwrap_or_else(|| "primary".to_string());
    match state.session_use_case.import_cookies(&session_id, &payload.cookies).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({ "success": true, "session_id": session_id }))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e.to_string() }))),
    }
}

// Handlers: Posts
pub async fn list_posts(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(50).clamp(1, 200);
    let offset = params.offset.unwrap_or(0).max(0);

    match state.post_repo.list_posts(limit, offset).await {
        Ok(posts) => (StatusCode::OK, Json(serde_json::json!({ "posts": posts, "limit": limit, "offset": offset }))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))),
    }
}

// Handlers: Leads
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

// Handlers: Outbox
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

// Health Probes
pub async fn health_liveness() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({ "status": "UP", "version": "v3.0.0" })))
}

pub async fn health_readiness(State(state): State<AppState>) -> impl IntoResponse {
    // Check DB connection
    match state.target_repo.get_all_targets().await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({ "status": "READY", "database": "CONNECTED" }))),
        Err(e) => (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({ "status": "NOT_READY", "error": e.to_string() }))),
    }
}

pub async fn metrics() -> impl IntoResponse {
    // Basic Prometheus formatted metrics
    let body = format!(
        "# HELP social_listener_up Liveness indicator\n# TYPE social_listener_up gauge\nsocial_listener_up 1\n"
    );
    (StatusCode::OK, [("content-type", "text/plain; version=0.0.4")], body)
}
