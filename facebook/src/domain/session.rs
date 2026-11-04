use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    Ready,
    LoginRequired,
    CheckpointRequired,
    Paused,
}

impl ToString for SessionStatus {
    fn to_string(&self) -> String {
        match self {
            SessionStatus::Ready => "READY".to_string(),
            SessionStatus::LoginRequired => "LOGIN_REQUIRED".to_string(),
            SessionStatus::CheckpointRequired => "CHECKPOINT_REQUIRED".to_string(),
            SessionStatus::Paused => "PAUSED".to_string(),
        }
    }
}

impl From<&str> for SessionStatus {
    fn from(s: &str) -> Self {
        match s {
            "READY" => SessionStatus::Ready,
            "LOGIN_REQUIRED" => SessionStatus::LoginRequired,
            "CHECKPOINT_REQUIRED" => SessionStatus::CheckpointRequired,
            _ => SessionStatus::Paused,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SocialSession {
    pub id: String,
    pub account_alias: String,
    pub platform: String,
    pub status: String,
    pub session_version: i32,
    pub lease_owner: Option<String>,
    pub lease_until: Option<DateTime<Utc>>,
    pub last_verified_at: Option<DateTime<Utc>>,
    pub incident_key: Option<String>,
    #[sqlx(default)]
    pub auth_failure_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl SocialSession {
    /// Kiểm tra phiên đăng nhập có đang hoạt động hợp lệ hay không:
    /// Status phải là READY và số lần lỗi auth liên tiếp phải dưới ngưỡng dừng (< 2).
    pub fn is_ready(&self) -> bool {
        self.status == "READY" && self.auth_failure_count < 2
    }
}
