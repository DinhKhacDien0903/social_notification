use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawPostDto {
    pub post_id: String,
    pub canonical_url: String,
    pub author_display: String,
    pub text: String,
    pub text_complete: bool,
    pub source_created_at: Option<DateTime<Utc>>,
}

#[async_trait]
pub trait FacebookEngine: Send + Sync {
    /// Cào danh sách bài viết từ một nhóm Facebook (sử dụng Facebook CLI hoặc CDP trực tiếp)
    async fn fetch_group_posts(
        &self,
        group_id: &str,
        max_posts: usize,
    ) -> Result<Vec<RawPostDto>, Box<dyn std::error::Error + Send + Sync>>;

    /// Kiểm tra trạng thái phiên Facebook
    async fn check_session_health(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiClassificationResult {
    pub is_lead: bool,
    pub intent: String,
    pub confidence: f32,
    pub reason: String,
    pub location: Option<String>,
    pub requested_date: Option<String>,
    pub summary: String,
}

#[async_trait]
pub trait AiGateway: Send + Sync {
    async fn classify_post(
        &self,
        text: &str,
    ) -> Result<AiClassificationResult, Box<dyn std::error::Error + Send + Sync>>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthillLeadPayload {
    pub event_key: String,
    pub platform: String,
    pub group_id: String,
    pub post_id: String,
    pub author: String,
    pub canonical_url: String,
    pub content: String,
    pub intent: String,
    pub confidence: f32,
    pub location: Option<String>,
    pub requested_date: Option<String>,
    pub summary: String,
    pub created_at: String,
}

#[async_trait]
pub trait AnthillNotifier: Send + Sync {
    async fn send_lead(
        &self,
        event_key: &str,
        payload: &AnthillLeadPayload,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>>;
}

pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

#[derive(Default)]
pub struct SystemClock;
impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}
