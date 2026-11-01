use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WatchTarget {
    pub id: i64,
    pub platform: String,
    pub group_id: String,
    pub display_name: String,
    pub session_id: String,
    pub enabled: bool,
    pub interval_seconds: i32,
    pub next_due_at: DateTime<Utc>,
    pub ordering: String,
    pub watermark: Option<DateTime<Utc>>,
    pub last_success_at: Option<DateTime<Utc>>,
    pub failure_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewWatchTarget {
    pub platform: String,
    pub group_id: String,
    pub display_name: String,
    pub session_id: String,
    pub enabled: bool,
    pub interval_seconds: i32,
    pub ordering: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CrawlRun {
    pub run_id: String,
    pub target_id: i64,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: String,
    pub observed_count: i32,
    pub new_count: i32,
    pub stop_reason: Option<String>,
    pub coverage: String,
    pub error_code: Option<String>,
    pub parser_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewCrawlRun {
    pub run_id: String,
    pub target_id: i64,
    pub status: String,
    pub observed_count: i32,
    pub new_count: i32,
    pub stop_reason: Option<String>,
    pub coverage: String,
    pub error_code: Option<String>,
    pub parser_version: String,
}
