use std::sync::Arc;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::domain::{SessionRepository, SocialSession, TargetRepository};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CookieItem {
    pub name: String,
    pub value: String,
    pub domain: Option<String>,
    pub path: Option<String>,
}

pub struct ManageSessionUseCase {
    session_repo: Arc<dyn SessionRepository>,
    target_repo: Arc<dyn TargetRepository>,
    profile_dir: String,
    fb_cdp: String,
}

impl ManageSessionUseCase {
    pub fn new(
        session_repo: Arc<dyn SessionRepository>,
        target_repo: Arc<dyn TargetRepository>,
        profile_dir: String,
        fb_cdp: String,
    ) -> Self {
        Self {
            session_repo,
            target_repo,
            profile_dir,
            fb_cdp,
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

        // KIỂM TRA ĐIỀU KIỆN SESSION HỢP LỆ:
        // Cookie bắt buộc phải có c_user (chứa ID người dùng) và xs (secret session token).
        let has_c_user = cookies.iter().any(|c| c.name == "c_user" && !c.value.trim().is_empty());
        let has_xs = cookies.iter().any(|c| c.name == "xs" && !c.value.trim().is_empty());

        if !has_c_user || !has_xs {
            return Err("Cookie không hợp lệ: Bắt buộc phải có cookie 'c_user' và 'xs' từ Facebook.".into());
        }

        // Lưu cookie vào file cache profile để CDP hoặc fb binary đọc
        let profile_path = std::path::Path::new(&self.profile_dir);
        if !profile_path.exists() {
            let _ = std::fs::create_dir_all(profile_path);
        }

        let cookie_file = profile_path.join(format!("{}_cookies.json", session_id));
        std::fs::write(&cookie_file, cookies_json)?;

        // Gọi CDP script nạp trực tiếp cookies vào Chrome in-memory cookie jar sạch sẽ qua CDP
        // (Storage.clearCookies -> Storage.setCookies), tuyệt đối không ghi đè SQLite khi Chrome đang chạy
        let inject_script = "/app/bin/inject_cookies.py";
        if std::path::Path::new(inject_script).exists() {
            let inject_res = tokio::process::Command::new("python3")
                .arg(inject_script)
                .arg(&cookie_file)
                .output()
                .await;
            if let Ok(out) = inject_res {
                info!("CDP inject_cookies status: {}", out.status);
            }
        }

        // Dừng fb daemon nếu có để apply cookie mới cho các lượt quét tiếp theo
        let _ = tokio::process::Command::new("/app/bin/fb")
            .arg("stop")
            .env("FB_PROFILE", &self.profile_dir)
            .env("FB_CDP", &self.fb_cdp)
            .output()
            .await;

        // Kích hoạt ngay tab neo Facebook và gửi presence ping đầu tiên để kết nối Comet WebSocket trong background (không chặn API response)
        if std::path::Path::new(inject_script).exists() {
            let script_path = inject_script.to_string();
            tokio::spawn(async move {
                let _ = tokio::process::Command::new("python3")
                    .arg(script_path)
                    .arg("--heartbeat")
                    .output()
                    .await;
            });
        }

        // Cập nhật CSDL: Reset auth_failure_count về 0, chuyển status sang READY
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
            auth_failure_count: 0,
            created_at: now,
            updated_at: now,
        };

        self.session_repo.upsert_session(&session).await?;
        self.session_repo.reset_auth_failure(session_id).await?;

        // Reset toàn bộ lỗi và lịch quét cho các nhóm đang bật để scheduler tiếp tục crawl ngay lập tức
        let _ = self.target_repo.reset_all_target_failures().await;
        info!(
            "Session '{}' updated to READY (v{}), auth_failure_count reset to 0, targets rescheduled. Crawling resumed.",
            session_id, version
        );

        Ok(())
    }
}
