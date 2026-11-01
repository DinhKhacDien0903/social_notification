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
    fb_cdp: String,
}

impl ManageSessionUseCase {
    pub fn new(session_repo: Arc<dyn SessionRepository>, profile_dir: String, fb_cdp: String) -> Self {
        Self {
            session_repo,
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

        // Lưu cookie vào file cache profile để CDP hoặc fb binary đọc
        let profile_path = std::path::Path::new(&self.profile_dir);
        if !profile_path.exists() {
            let _ = std::fs::create_dir_all(profile_path);
        }

        let cookie_file = profile_path.join(format!("{}_cookies.json", session_id));
        std::fs::write(&cookie_file, cookies_json)?;

        // Gọi CDP script nạp trực tiếp cookies vào Chrome in-memory cookie jar
        let inject_script = "/app/bin/inject_cookies.py";
        if std::path::Path::new(inject_script).exists() {
            let inject_res = tokio::process::Command::new("python3")
                .arg(inject_script)
                .output()
                .await;
            if let Ok(out) = inject_res {
                info!("CDP inject_cookies status: {}", out.status);
            }
        }

        // Đồng bộ cookie trực tiếp vào CSDL SQLite Cookies của Chrome
        let _ = Self::sync_cookies_to_chrome_db(&cookies).await;

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

    async fn sync_cookies_to_chrome_db(cookies: &[CookieItem]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let db_paths = vec![
            std::path::PathBuf::from("/root/.cache/fb/chrome/Default/Cookies"),
            std::path::PathBuf::from("/app/runtime/profile/chrome/Default/Cookies"),
        ];

        for path in db_paths {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            let db_url = format!("sqlite://{}?mode=rwc", path.to_string_lossy());
            if let Ok(pool) = sqlx::sqlite::SqlitePoolOptions::new().connect(&db_url).await {
                let _ = sqlx::query(r#"
                    CREATE TABLE IF NOT EXISTS cookies (
                        creation_utc INTEGER NOT NULL,
                        host_key TEXT NOT NULL,
                        top_frame_site_key TEXT NOT NULL DEFAULT '',
                        name TEXT NOT NULL,
                        value TEXT NOT NULL,
                        encrypted_value BLOB NOT NULL DEFAULT X'',
                        path TEXT NOT NULL DEFAULT '/',
                        expires_utc INTEGER NOT NULL DEFAULT 0,
                        is_secure INTEGER NOT NULL DEFAULT 1,
                        is_httponly INTEGER NOT NULL DEFAULT 0,
                        last_access_utc INTEGER NOT NULL DEFAULT 0,
                        has_expires INTEGER NOT NULL DEFAULT 1,
                        is_persistent INTEGER NOT NULL DEFAULT 1,
                        priority INTEGER NOT NULL DEFAULT 1,
                        samesite INTEGER NOT NULL DEFAULT 0,
                        source_scheme INTEGER NOT NULL DEFAULT 2,
                        source_port INTEGER NOT NULL DEFAULT 443,
                        has_cross_site_ancestor INTEGER NOT NULL DEFAULT 0,
                        last_update_utc INTEGER NOT NULL DEFAULT 0,
                        source_type INTEGER NOT NULL DEFAULT 1,
                        PRIMARY KEY (host_key, top_frame_site_key, name, path)
                    )
                "#).execute(&pool).await;

                let now_micro = chrono::Utc::now().timestamp_micros();
                let chrome_now = now_micro + 11644473600_000_000i64;
                let chrome_expires = chrome_now + (365 * 86400 * 1_000_000i64);

                for c in cookies {
                    let domain = c.domain.as_deref().unwrap_or(".facebook.com");
                    let path_val = c.path.as_deref().unwrap_or("/");
                    let is_httponly = if c.name == "xs" || c.name == "datr" || c.name == "fr" || c.name == "sb" { 1 } else { 0 };

                    let _ = sqlx::query(r#"
                        INSERT OR REPLACE INTO cookies (
                            creation_utc, host_key, top_frame_site_key, name, value, encrypted_value,
                            path, expires_utc, is_secure, is_httponly, last_access_utc, has_expires,
                            is_persistent, priority, samesite, source_scheme, source_port, has_cross_site_ancestor,
                            last_update_utc, source_type
                        ) VALUES (
                            ?, ?, '', ?, ?, X'',
                            ?, ?, 1, ?, ?, 1,
                            1, 1, 0, 2, 443, 0,
                            ?, 1
                        )
                    "#)
                    .bind(chrome_now)
                    .bind(domain)
                    .bind(&c.name)
                    .bind(&c.value)
                    .bind(path_val)
                    .bind(chrome_expires)
                    .bind(is_httponly)
                    .bind(chrome_now)
                    .bind(chrome_now)
                    .execute(&pool)
                    .await;
                }

                let _ = pool.close().await;
                info!("Synced {} cookies to Chrome DB at {}", cookies.len(), path.display());
            }
        }

        // Restart daemon to apply cookies
        let _ = tokio::process::Command::new("/app/bin/fb").arg("quit").output().await;

        Ok(())
    }
}
