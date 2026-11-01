use axum::{
    routing::{get, post, put},
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use super::handlers::{
    create_target, get_session_status, get_system_status, health_freshness, health_liveness,
    health_readiness, import_session_cookies, list_leads, list_outbox, list_posts, list_runs,
    list_targets, metrics, retry_outbox_event, toggle_target, AppState,
};

pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // Targets API
        .route("/api/targets", get(list_targets).post(create_target))
        .route("/api/targets/:id/toggle", put(toggle_target))
        // Session Cookie Sync & Status API
        .route("/api/session/status", get(get_session_status))
        .route("/api/session/import", post(import_session_cookies))
        // System Status & Runs API
        .route("/api/status", get(get_system_status))
        .route("/api/runs", get(list_runs))
        // Posts & Leads API
        .route("/api/posts", get(list_posts))
        .route("/api/leads", get(list_leads))
        // Outbox API
        .route("/api/outbox", get(list_outbox))
        .route("/api/outbox/:id/retry", post(retry_outbox_event))
        // Health Probes & Metrics
        .route("/health/liveness", get(health_liveness))
        .route("/health/readiness", get(health_readiness))
        .route("/health/freshness", get(health_freshness))
        .route("/metrics", get(metrics))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
