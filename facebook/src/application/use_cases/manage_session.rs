use std::sync::Arc;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::domain::{SessionRepository, SocialSession};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CookieItem {
    pub name: String,
    pub value: String,
    pub domain: Option<String>,
    pub path: Option<String>,
}

pub struct ManageSessionUseCase {
    session_repo: Arc<dyn SessionRepository>,
    profile_dir: String,
}

impl ManageSessionUseCase {
    pub fn new(session_repo: Arc<dyn SessionRepository>, profile_dir: String) -> Self {
        Self {
            session_repo,
            profile_dir,
        }
    }

    pub async fn import_cookies(
        &self,
        session_id: &str,
        cookies_json: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Parse cookie JSON để đảm bảo tính hợp lệ
        let cookies: Vec<CookieItem> = serde_json::from_str(cookies_json)?;
        info!("Importing {} cookies for session '{}'", cookies.len(), session_id);

        // Lưu cookie vào file cache profile để CDP hoặc fb binary đọc
        let profile_path = std::path::Path::new(&self.profile_dir);
        if !profile_path.exists() {
            let _ = std::fs::create_dir_all(profile_path);
        }

        let cookie_file = profile_path.join(format!("{}_cookies.json", session_id));
        std::fs::write(&cookie_file, cookies_json)?;

        // Cập nhật CSDL
        let now = Utc::now();
        let existing = self.session_repo.get_session(session_id).await?;
        let version = existing.as_ref().map_or(1, |s| s.session_version + 1);

        let session = SocialSession {
            id: session_id.to_string(),
            account_alias: existing.as_ref().map_or_else(|| "default_bot".to_string(), |s| s.account_alias.clone()),
            platform: "facebook".to_string(),
            status: "READY".to_string(),
            session_version: version,
            lease_owner: None,
            lease_until: None,
            last_verified_at: Some(now),
            incident_key: None,
            created_at: now,
            updated_at: now,
        };

        self.session_repo.upsert_session(&session).await?;
        info!("Session '{}' updated to READY (v{})", session_id, version);

        Ok(())
    }
}
