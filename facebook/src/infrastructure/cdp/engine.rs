use async_trait::async_trait;
use regex::Regex;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, error, info, warn};

use crate::application::interfaces::{FacebookEngine, RawPostDto};

pub struct FacebookCliEngine {
    fb_bin_path: String,
    profile_dir: String,
    fb_cdp: String,
    url_regex: Regex,
}

impl FacebookCliEngine {
    pub fn new(fb_bin_path: String, profile_dir: String, fb_cdp: String) -> Self {
        let url_regex = Regex::new(r"/(?:groups/[^/]+/posts/|posts/|permalink\.php\?story_fbid=)(\d+)").unwrap();
        Self {
            fb_bin_path,
            profile_dir,
            fb_cdp,
            url_regex,
        }
    }

    /// Trích xuất post_id từ URL Facebook
    fn extract_post_id(&self, url: &str) -> Option<String> {
        self.url_regex.captures(url).and_then(|cap| cap.get(1)).map(|m| m.as_str().to_string())
    }

    /// Tự động phân giải URL WebSocket của Chrome CDP nếu FB_CDP chỉ là cổng hoặc chuỗi cục bộ
    async fn resolve_cdp_target(fb_cdp: &str) -> String {
        if fb_cdp.starts_with("ws://") || fb_cdp.starts_with("wss://") {
            return fb_cdp.to_string();
        }
        let port = fb_cdp.trim_start_matches(':');
        let client = reqwest::Client::new();
        let version_url = format!("http://127.0.0.1:{}/json/version", port);
        if let Ok(resp) = client
            .get(&version_url)
            .header("Host", format!("127.0.0.1:{}", port))
            .send()
            .await
        {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                if let Some(ws) = json.get("webSocketDebuggerUrl").and_then(|w| w.as_str()) {
                    info!("Dynamically resolved Chrome CDP WebSocket URL: {}", ws);
                    return ws.to_string();
                }
            }
        }
        fb_cdp.to_string()
    }

    /// Đóng các tab Facebook mở ra trong Chrome CDP sau khi crawl xong để giải phóng CPU và RAM
    async fn cleanup_cdp_tabs(fb_cdp: &str) {
        let port = if let Some(pos) = fb_cdp.find("127.0.0.1:") {
            let after = &fb_cdp[pos + "127.0.0.1:".len()..];
            after.split('/').next().unwrap_or("9222")
        } else if fb_cdp.chars().all(|c| c.is_ascii_digit()) {
            fb_cdp
        } else {
            "9222"
        };

        let client = reqwest::Client::new();
        let list_url = format!("http://127.0.0.1:{}/json/list", port);
        if let Ok(resp) = client
            .get(&list_url)
            .header("Host", format!("127.0.0.1:{}", port))
            .send()
            .await
        {
            if let Ok(pages) = resp.json::<Vec<serde_json::Value>>().await {
                for p in pages {
                    if p.get("type").and_then(|t| t.as_str()) == Some("page") {
                        if let Some(id) = p.get("id").and_then(|i| i.as_str()) {
                            let close_url = format!("http://127.0.0.1:{}/json/close/{}", port, id);
                            let _ = client
                                .get(&close_url)
                                .header("Host", format!("127.0.0.1:{}", port))
                                .send()
                                .await;
                        }
                    }
                }
                debug!("Cleaned up lingering Chrome CDP page tabs");
            }
        }
    }

    /// Phân tích chuỗi thời gian bài viết Facebook sang DateTime<Utc>
    fn parse_facebook_time(time_str: &str) -> Option<chrono::DateTime<chrono::Utc>> {
        let s = time_str.trim().to_lowercase();
        if s.is_empty() || s == "-" {
            return None;
        }
        let now = chrono::Utc::now();
        if s == "vừa xong" || s == "just now" {
            return Some(now);
        }
        // "x phút" / "x phút trước" / "x mins"
        if let Some(m) = regex::Regex::new(r"(\d+)\s*(?:phút|min)").ok().and_then(|r| r.captures(&s)) {
            if let Ok(mins) = m[1].parse::<i64>() {
                return Some(now - chrono::Duration::minutes(mins));
            }
        }
        // "x giờ" / "x giờ trước" / "x hrs" / "x h"
        if let Some(m) = regex::Regex::new(r"(\d+)\s*(?:giờ|hr|h\b)").ok().and_then(|r| r.captures(&s)) {
            if let Ok(hours) = m[1].parse::<i64>() {
                return Some(now - chrono::Duration::hours(hours));
            }
        }
        // "hôm qua lúc HH:MM"
        if let Some(m) = regex::Regex::new(r"hôm qua\s+lúc\s+(\d+):(\d+)").ok().and_then(|r| r.captures(&s)) {
            let hour: u32 = m[1].parse().unwrap_or(0);
            let min: u32 = m[2].parse().unwrap_or(0);
            use chrono::{Datelike, TimeZone};
            let yesterday = (now + chrono::Duration::hours(7)) - chrono::Duration::days(1);
            if let Some(naive_date) = chrono::NaiveDate::from_ymd_opt(yesterday.year(), yesterday.month(), yesterday.day()) {
                if let Some(naive_dt) = naive_date.and_hms_opt(hour, min, 0) {
                    return Some(chrono::Utc.from_utc_datetime(&(naive_dt - chrono::Duration::hours(7))));
                }
            }
        }
        // "x ngày" / "x ngày trước" / "x days"
        if let Some(m) = regex::Regex::new(r"(\d+)\s*(?:ngày|day)").ok().and_then(|r| r.captures(&s)) {
            if let Ok(days) = m[1].parse::<i64>() {
                return Some(now - chrono::Duration::days(days));
            }
        }
        // "31 tháng 10 lúc 18:15" hoặc "31 tháng 10" (có thể có năm: "31 tháng 10, 2025" / "31 tháng 10 năm 2025")
        if let Some(m) = regex::Regex::new(r"(\d+)\s*tháng\s*(\d+)(?:\s*(?:năm|,)\s*(\d{4}))?(?:\s*lúc\s*(\d+):(\d+))?").ok().and_then(|r| r.captures(&s)) {
            let day: u32 = m[1].parse().unwrap_or(1);
            let month: u32 = m[2].parse().unwrap_or(1);
            use chrono::{Datelike, TimeZone};
            let vn_now = now + chrono::Duration::hours(7);
            let year = m.get(3).and_then(|y| y.as_str().parse().ok()).unwrap_or(vn_now.year());
            let hour: u32 = m.get(4).and_then(|h| h.as_str().parse().ok()).unwrap_or(0);
            let min: u32 = m.get(5).and_then(|min| min.as_str().parse().ok()).unwrap_or(0);
            if let Some(naive_date) = chrono::NaiveDate::from_ymd_opt(year, month, day) {
                if let Some(naive_dt) = naive_date.and_hms_opt(hour, min, 0) {
                    return Some(chrono::Utc.from_utc_datetime(&(naive_dt - chrono::Duration::hours(7))));
                }
            }
        }
        None
    }
}

#[async_trait]
impl FacebookEngine for FacebookCliEngine {
    async fn fetch_group_posts(
        &self,
        group_id: &str,
        max_posts: usize,
    ) -> Result<Vec<RawPostDto>, Box<dyn std::error::Error + Send + Sync>> {
        let formatted_target = if group_id.starts_with("/groups/") {
            group_id.to_string()
        } else if group_id.chars().all(|c| c.is_ascii_digit()) {
            format!("/groups/{}", group_id)
        } else {
            format!("/groups/{}", group_id)
        };

        info!("Executing facebook-cli for group: {} (max: {})", formatted_target, max_posts);

        // Phân giải WebSocket URL chính xác từ Chrome CDP
        let cdp_target = Self::resolve_cdp_target(&self.fb_cdp).await;

        let mut cmd = Command::new(&self.fb_bin_path);
        cmd.arg("posts")
            .arg(&formatted_target)
            .arg("-n")
            .arg(max_posts.to_string())
            .arg("--urls")
            .arg("--full")
            .env("FB_PROFILE", &self.profile_dir)
            .env("FB_CDP", &cdp_target)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = match cmd.output().await {
            Ok(out) => out,
            Err(e) => {
                error!("Failed to launch facebook-cli binary at {}: {}", self.fb_bin_path, e);
                return Err(format!("Cannot execute facebook-cli: {}", e).into());
            }
        };

        // Sau khi crawl xong, đóng toàn bộ tab Facebook để tránh ngốn CPU
        Self::cleanup_cdp_tabs(&cdp_target).await;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("facebook-cli stderr: {}", stderr);
            return Err(format!("facebook-cli exited with error: {}", stderr).into());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut results = Vec::new();

        // Từng dòng TSV từ `fb posts`:
        // ref \t author \t time \t reactions/comments \t text \t url (khi có --urls)
        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.split('\t').collect();
            if parts.is_empty() {
                continue;
            }

            let (ref_id, author, _time_str, _stats, text, url) = match parts.len() {
                len if len >= 6 => (parts[0], parts[1], parts[2], parts[3], parts[4], parts[5]),
                5 => (parts[0], parts[1], parts[2], parts[3], parts[4], ""),
                4 => (parts[0], parts[1], parts[2], "", parts[3], ""),
                3 => (parts[0], parts[1], "", "", parts[2], ""),
                2 => (parts[0], "", "", "", parts[1], ""),
                _ => (parts[0], "", "", "", "", ""),
            };

            let clean_post_id = if let Some(id) = self.extract_post_id(url) {
                id
            } else if let Some(id) = self.extract_post_id(ref_id) {
                id
            } else if ref_id.is_empty() || ref_id == "-" {
                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(author.as_bytes());
                hasher.update(text.as_bytes());
                let hash = format!("{:x}", hasher.finalize());
                format!("anon_{}", &hash[..12])
            } else {
                ref_id.trim_matches('/').replace('/', "_")
            };

            let canonical_url = if !url.is_empty() && url.starts_with("http") {
                url.to_string()
            } else if clean_post_id.starts_with("anon_") {
                format!("https://www.facebook.com/groups/{}/", group_id)
            } else {
                format!("https://www.facebook.com/groups/{}/posts/{}/", group_id, clean_post_id)
            };

            let source_created_at = Self::parse_facebook_time(_time_str);

            results.push(RawPostDto {
                post_id: clean_post_id,
                canonical_url,
                author_display: author.to_string(),
                text: text.to_string(),
                text_complete: true,
                source_created_at,
            });
        }

        debug!("Extracted {} raw posts from facebook-cli", results.len());
        Ok(results)
    }

    async fn check_session_health(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let cdp_target = Self::resolve_cdp_target(&self.fb_cdp).await;
        let mut cmd = Command::new(&self.fb_bin_path);
        cmd.arg("status")
            .env("FB_PROFILE", &self.profile_dir)
            .env("FB_CDP", &cdp_target)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = cmd.output().await?;
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            Ok(!stdout.contains("LOGIN_REQUIRED") && !stdout.contains("CHECKPOINT") && !stdout.contains("not logged in"))
        } else {
            Ok(false)
        }
    }
}
