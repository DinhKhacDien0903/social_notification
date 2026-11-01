use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutboxStatus {
    Pending,
    Processing,
    Delivered,
    DeadLetter,
}

impl ToString for OutboxStatus {
    fn to_string(&self) -> String {
        match self {
            OutboxStatus::Pending => "PENDING".to_string(),
            OutboxStatus::Processing => "PROCESSING".to_string(),
            OutboxStatus::Delivered => "DELIVERED".to_string(),
            OutboxStatus::DeadLetter => "DEAD_LETTER".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct OutboxEvent {
    pub id: i64,
    pub event_key: String,
    pub payload: String,
    pub status: String,
    pub available_at: DateTime<Utc>,
    pub attempts: i32,
    pub lease_owner: Option<String>,
    pub lease_until: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub remote_message_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewOutboxEvent {
    pub event_key: String,
    pub payload: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Job {
    pub id: i64,
    pub kind: String,
    pub post_id: i64,
    pub revision: i32,
    pub status: String,
    pub available_at: DateTime<Utc>,
    pub attempts: i32,
    pub lease_owner: Option<String>,
    pub lease_until: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewJob {
    pub kind: String,
    pub post_id: i64,
    pub revision: i32,
}
