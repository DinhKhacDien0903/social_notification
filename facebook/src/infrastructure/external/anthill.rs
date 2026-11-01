use async_trait::async_trait;
use tracing::info;

use crate::application::interfaces::{AnthillLeadPayload, AnthillNotifier};

pub struct AnthillClient {
    webhook_url: String,
    token: String,
    client: reqwest::Client,
}

impl AnthillClient {
    pub fn new(webhook_url: String, token: String) -> Self {
        Self {
            webhook_url,
            token,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
        }
    }
}

#[async_trait]
impl AnthillNotifier for AnthillClient {
    async fn send_lead(
        &self,
        event_key: &str,
        payload: &AnthillLeadPayload,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        info!("Sending lead webhook to {} with Idempotency-Key: {}", self.webhook_url, event_key);

        let mut req = self.client.post(&self.webhook_url)
            .header("Content-Type", "application/json")
            .header("Idempotency-Key", event_key)
            .json(payload);

        if !self.token.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", self.token));
        }

        let resp = req.send().await?;
        let status = resp.status();

        if status.is_success() {
            let msg_id = resp.headers()
                .get("x-message-id")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            Ok(msg_id)
        } else {
            let err_body = resp.text().await.unwrap_or_default();
            Err(format!("Anthill webhook responded with HTTP {}: {}", status, err_body).into())
        }
    }
}
