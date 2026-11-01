use std::sync::Arc;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

mod domain;
mod application;
mod infrastructure;
mod presentation;

use application::classifier::{ClassifierService, GeminiClassifier, RuleClassifier};
use application::use_cases::{ClassifyJobsUseCase, CrawlTargetUseCase, DispatchOutboxUseCase, ManageSessionUseCase};
use domain::{NewWatchTarget, SessionRepository, SocialSession, TargetRepository};
use infrastructure::{AnthillClient, AppConfig, FacebookCliEngine, SqliteDatabase};
use presentation::{create_router, run_outbox_worker, run_pipeline_worker, run_scheduler_worker, AppState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Khởi tạo logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "social_listener=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting Social Listener v3.0.0 (100% Pure Rust Architecture)");

    // 2. Tải cấu hình
    let config = AppConfig::from_env();
    info!("Configuration loaded. Database: {}", config.database_url);

    // 3. Khởi tạo kết nối CSDL SQLite
    let connect_opts = SqliteConnectOptions::from_str(&config.database_url)?
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .connect_with(connect_opts)
        .await?;

    info!("Database connection established. Running migrations...");

    // 4. Chạy migrations schema 8 bảng
    const MIGRATION_SQL: &str = include_str!("../migrations/20261030000001_initial_schema.sql");
    sqlx::raw_sql(MIGRATION_SQL).execute(&pool).await?;
    info!("Database migrations applied successfully.");

    let db = SqliteDatabase::new(pool);
    let target_repo = Arc::new(db.clone());
    let post_repo = Arc::new(db.clone());
    let job_repo = Arc::new(db.clone());
    let lead_repo = Arc::new(db.clone());
    let outbox_repo = Arc::new(db.clone());
    let session_repo = Arc::new(db.clone());

    // Seed session 'primary' và target mặc định nếu rỗng
    if session_repo.get_session("primary").await?.is_none() {
        let initial_session = SocialSession {
            id: "primary".to_string(),
            account_alias: "Primary Bot Account".to_string(),
            platform: "facebook".to_string(),
            status: "READY".to_string(),
            session_version: 1,
            lease_owner: None,
            lease_until: None,
            last_verified_at: Some(chrono::Utc::now()),
            incident_key: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        session_repo.upsert_session(&initial_session).await?;
        info!("Seeded default primary session");
    }

    // Seed duy nhất nhóm Facebook mục tiêu: https://www.facebook.com/groups/690021959247586/
    let pool_ref = db.pool();
    let target_69: Option<domain::WatchTarget> = sqlx::query_as(
        "SELECT * FROM social_watch_targets WHERE group_id = '690021959247586'"
    )
    .fetch_optional(pool_ref)
    .await?;

    if target_69.is_none() {
        let default_target = NewWatchTarget {
            platform: "facebook".to_string(),
            group_id: "690021959247586".to_string(),
            display_name: "Hội Đam Mê Trekking & Leo Núi Việt Nam".to_string(),
            session_id: "primary".to_string(),
            enabled: true,
            interval_seconds: config.crawl_interval_seconds,
            ordering: "chronological".to_string(),
        };
        target_repo.create_target(default_target).await?;
        info!("Seeded target: https://www.facebook.com/groups/690021959247586/");
    }

    // Cập nhật interval 180s (3 phút) và kích hoạt quét ngay cho nhóm 690021959247586
    sqlx::query("UPDATE social_watch_targets SET interval_seconds = ?, next_due_at = CURRENT_TIMESTAMP, failure_count = 0, enabled = 1 WHERE group_id = '690021959247586'")
        .bind(config.crawl_interval_seconds)
        .execute(pool_ref)
        .await?;

    // Xóa các target mẫu khác (không dùng data seed nữa, chỉ giữ nhóm 690021959247586)
    sqlx::query("DELETE FROM social_watch_targets WHERE group_id != '690021959247586'")
        .execute(pool_ref)
        .await?;

    // 5. Khởi tạo Adapters ngoại vi
    let fb_engine = Arc::new(FacebookCliEngine::new(
        config.fb_bin_path.clone(),
        config.chrome_profile_dir.clone(),
        config.fb_cdp.clone(),
    ));

    let gemini_ai = Arc::new(GeminiClassifier::new(config.gemini_api_key.clone()));
    let rule_classifier = Arc::new(RuleClassifier::new());
    let classifier_service = Arc::new(ClassifierService::new(rule_classifier, gemini_ai));

    let anthill_notifier = Arc::new(AnthillClient::new(
        config.to_kien_webhook_url.clone(),
        config.to_kien_token.clone(),
    ));

    // 6. Khởi tạo Use Cases
    let crawl_use_case = Arc::new(CrawlTargetUseCase::new(
        fb_engine.clone(),
        target_repo.clone(),
        post_repo.clone(),
        job_repo.clone(),
    ));

    let worker_uuid = Uuid::new_v4().to_string();

    let classify_use_case = Arc::new(ClassifyJobsUseCase::new(
        post_repo.clone(),
        job_repo.clone(),
        lead_repo.clone(),
        outbox_repo.clone(),
        classifier_service.clone(),
        format!("worker-classifier-{}", worker_uuid),
    ));

    let dispatch_use_case = Arc::new(DispatchOutboxUseCase::new(
        outbox_repo.clone(),
        anthill_notifier.clone(),
        format!("worker-outbox-{}", worker_uuid),
    ));

    let session_use_case = Arc::new(ManageSessionUseCase::new(
        session_repo.clone(),
        config.chrome_profile_dir.clone(),
        config.fb_cdp.clone(),
    ));

    // 7. Kích hoạt 3 background workers (Tokio tasks)
    let scheduler_target_repo = target_repo.clone();
    let scheduler_crawl_uc = crawl_use_case.clone();
    let scheduler_worker_id = format!("worker-scheduler-{}", worker_uuid);
    tokio::spawn(async move {
        run_scheduler_worker(scheduler_target_repo, scheduler_crawl_uc, scheduler_worker_id).await;
    });

    let pipeline_classify_uc = classify_use_case.clone();
    tokio::spawn(async move {
        run_pipeline_worker(pipeline_classify_uc).await;
    });

    let outbox_dispatch_uc = dispatch_use_case.clone();
    tokio::spawn(async move {
        run_outbox_worker(outbox_dispatch_uc).await;
    });

    info!("All 3 background workers spawned successfully (Scheduler, Classifier Pipeline, Outbox Dispatcher)");

    // 8. Khởi chạy Axum REST API Server
    let app_state = AppState {
        db: db.clone(),
        target_repo,
        post_repo,
        lead_repo,
        outbox_repo,
        session_repo,
        session_use_case,
    };

    let router = create_router(app_state);
    let addr = format!("{}:{}", config.server_host, config.server_port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Axum Web API Server listening on http://{}", addr);

    axum::serve(listener, router).await?;

    Ok(())
}
