use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Post {
    pub id: i64,
    pub platform: String,
    pub group_id: String,
    pub post_id: String,
    pub canonical_url: String,
    pub identity_key: String,
    pub content_hash: String,
    pub text: String,
    pub text_complete: bool,
    pub author_display: String,
    pub source_created_at: Option<DateTime<Utc>>,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub revision: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewPost {
    pub platform: String,
    pub group_id: String,
    pub post_id: String,
    pub canonical_url: String,
    pub identity_key: String,
    pub content_hash: String,
    pub text: String,
    pub text_complete: bool,
    pub author_display: String,
    pub source_created_at: Option<DateTime<Utc>>,
}

impl Post {
    pub fn compute_identity_key(platform: &str, group_id: &str, post_id: &str) -> String {
        format!("{}:{}:{}", platform, group_id, post_id)
    }
}
