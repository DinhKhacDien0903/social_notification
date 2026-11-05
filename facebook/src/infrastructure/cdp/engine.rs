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
    crawl_count: std::sync::atomic::AtomicUsize,
}

impl FacebookCliEngine {
    pub fn new(fb_bin_path: String, profile_dir: String, fb_cdp: String) -> Self {
        let url_regex = Regex::new(r"/(?:groups/[^/]+/posts/|posts/|permalink\.php\?story_fbid=)(\d+)").unwrap();
        Self {
            fb_bin_path,
            profile_dir,
            fb_cdp,
            url_regex,
            crawl_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Trích xuất post_id từ URL Facebook hoặc ref_id
    fn extract_post_id(&self, text: &str) -> Option<String> {
        if text.is_empty() {
            return None;
        }

        // 1. Nhận diện set=gm.(\d+) hoặc set=pcb.(\d+) từ link ảnh trong nhóm Facebook
        let gm_regex = Regex::new(r"set=(?:gm|pcb)\.(\d+)").ok()?;
        if let Some(caps) = gm_regex.captures(text) {
            if let Some(m) = caps.get(1) {
                return Some(m.as_str().to_string());
            }
        }

        // 2. Nhận diện multi_permalinks=(\d+)
        let mp_regex = Regex::new(r"multi_permalinks=(\d+)").ok()?;
        if let Some(caps) = mp_regex.captures(text) {
            if let Some(m) = caps.get(1) {
                return Some(m.as_str().to_string());
            }
        }

        // 3. Regex chuẩn: /groups/.../posts/(\d+), /posts/(\d+), permalink.php?story_fbid=(\d+)
        if let Some(caps) = self.url_regex.captures(text) {
            if let Some(m) = caps.get(1) {
                return Some(m.as_str().to_string());
            }
        }

        // 4. story_fbid=(\d+)
        let story_fbid_regex = Regex::new(r"[?&]story_fbid=(\d+)").ok()?;
        if let Some(caps) = story_fbid_regex.captures(text) {
            if let Some(m) = caps.get(1) {
                return Some(m.as_str().to_string());
            }
        }

        None
    }

    /// Tự động phân giải URL WebSocket của Chrome CDP nếu FB_CDP chỉ là cổng hoặc chuỗi cục bộ
    async fn resolve_cdp_target(fb_cdp: &str) -> String {
        if fb_cdp.starts_with("ws://") || fb_cdp.starts_with("wss://") {
            return fb_cdp.to_string();
        }
        let port = fb_cdp.trim_start_matches(':');
        let client = reqwest::Client::new();
        let version_url = format!("http://127.0.0.1:{}/json/version", port);

        for attempt in 1..=3 {
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
            if attempt < 3 {
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            }
        }
        fb_cdp.to_string()
    }

    /// Đóng các tab trong Chrome CDP sau khi crawl xong để giải phóng triệt để CPU và RAM
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
        let host_header = format!("127.0.0.1:{}", port);

        let list_url = format!("http://127.0.0.1:{}/json/list", port);
        let pages = match client.get(&list_url).header("Host", &host_header).send().await {
            Ok(resp) => resp.json::<Vec<serde_json::Value>>().await.unwrap_or_default(),
            Err(e) => {
                debug!("Failed to query Chrome targets for cleanup: {}", e);
                return;
            }
        };

        // Tìm tab 'about:blank' đã có sẵn để làm neo (anchor), tránh mở thừa tab mới
        let mut anchor_id = pages.iter().find_map(|p| {
            let tab_type = p.get("type").and_then(|t| t.as_str()).unwrap_or("");
            let tab_url = p.get("url").and_then(|u| u.as_str()).unwrap_or("");
            if tab_type == "page" && tab_url == "about:blank" {
                p.get("id").and_then(|i| i.as_str()).map(|s| s.to_string())
            } else {
                None
            }
        });

        // Chỉ khi chưa có tab about:blank nào mới mở 1 tab duy nhất làm anchor để Chrome không thoát
        if anchor_id.is_none() {
            let new_tab_url = format!("http://127.0.0.1:{}/json/new?about:blank", port);
            if let Ok(resp) = client.put(&new_tab_url).header("Host", &host_header).send().await {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    anchor_id = json.get("id").and_then(|i| i.as_str()).map(|s| s.to_string());
                }
            }
        }

        // Đóng toàn bộ các tab type 'page' khác (Facebook, URL rỗng, duplicate about:blank)
        for p in &pages {
            let tab_type = p.get("type").and_then(|t| t.as_str()).unwrap_or("");
            let id = p.get("id").and_then(|i| i.as_str()).unwrap_or("");

            if tab_type == "page" && !id.is_empty() {
                if let Some(ref aid) = anchor_id {
                    if id == aid {
                        continue;
                    }
                }
                let close_url = format!("http://127.0.0.1:{}/json/close/{}", port, id);
                let _ = client.get(&close_url).header("Host", &host_header).send().await;
            }
        }
        debug!("Cleaned up lingering Chrome CDP tabs, kept anchor {:?}", anchor_id);
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

    /// Tái khởi động Chrome định kỳ để giải phóng 100% RAM tích tụ và ngắt các tiến trình rò rỉ
    pub async fn trigger_chrome_recycle() {
        let recycle_script = "/app/bin/recycle_chrome.sh";
        if std::path::Path::new(recycle_script).exists() {
            info!("Triggering scheduled Chrome process recycle via {}", recycle_script);
            let res = Command::new(recycle_script).output().await;
            match res {
                Ok(out) if out.status.success() => {
                    info!("Chrome process recycled successfully. RAM reset to baseline.");
                }
                Ok(out) => {
                    warn!("Chrome recycle finished with status {}: {}", out.status, String::from_utf8_lossy(&out.stderr));
                }
                Err(e) => {
                    warn!("Failed to execute Chrome recycle script: {}", e);
                }
            }
        }
    }
}

#[async_trait]
impl FacebookEngine for FacebookCliEngine {
    async fn fetch_group_posts(
        &self,
        group_id: &str,
        max_posts: usize,
    ) -> Result<Vec<RawPostDto>, Box<dyn std::error::Error + Send + Sync>> {
        let clean_id = group_id.trim();
        let base_path = if clean_id.starts_with("http://") || clean_id.starts_with("https://") {
            clean_id.to_string()
        } else if clean_id.starts_with("/groups/") {
            clean_id.to_string()
        } else if clean_id.starts_with("groups/") {
            format!("/{}", clean_id)
        } else {
            format!("/groups/{}", clean_id.trim_matches('/'))
        };

        // Bổ sung query sorting_setting=CHRONOLOGICAL để ép Facebook sắp xếp bài viết mới nhất theo thời gian thực (tránh bị kẹt ở "Phù hợp nhất")
        let formatted_target = if base_path.contains("/groups/") && !base_path.contains("sorting_setting=") {
            if base_path.contains('?') {
                format!("{}&sorting_setting=CHRONOLOGICAL", base_path)
            } else {
                format!("{}/?sorting_setting=CHRONOLOGICAL", base_path.trim_end_matches('/'))
            }
        } else {
            base_path
        };

        info!("Executing facebook-cli for group: {} (max: {})", formatted_target, max_posts);

        // Phân giải WebSocket URL chính xác từ Chrome CDP
        let cdp_target = Self::resolve_cdp_target(&self.fb_cdp).await;

        let fb_timeout = std::env::var("FB_TIMEOUT").unwrap_or_else(|_| "35".to_string());
        let timeout_secs: u64 = fb_timeout.parse().unwrap_or(35);
        let hard_timeout = std::time::Duration::from_secs(timeout_secs + 5);

        let mut cmd = Command::new(&self.fb_bin_path);
        cmd.arg("posts")
            .arg(&formatted_target)
            .arg("-n")
            .arg(max_posts.to_string())
            .arg("--urls")
            .env("FB_PROFILE", &self.profile_dir)
            .env("FB_CDP", &cdp_target)
            .env("FB_TIMEOUT", &fb_timeout)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let output_res = match tokio::time::timeout(hard_timeout, cmd.output()).await {
            Ok(res) => res,
            Err(_) => {
                warn!(
                    "CRAWL TIMEOUT (> {}s) for group {}. Aborting process and resetting CDP...",
                    hard_timeout.as_secs(),
                    formatted_target
                );
                // Dọn dẹp khẩn cấp tab Chrome và force stop fb daemon
                Self::cleanup_cdp_tabs(&cdp_target).await;
                let _ = Command::new(&self.fb_bin_path)
                    .arg("stop")
                    .env("FB_PROFILE", &self.profile_dir)
                    .env("FB_CDP", &cdp_target)
                    .output()
                    .await;
                return Err(format!("crawl timed out after {}s (fail-fast protection)", hard_timeout.as_secs()).into());
            }
        };

        // Luôn cleanup tab Chrome để giải phóng CPU và RAM dù lệnh có lỗi hay timeout
        Self::cleanup_cdp_tabs(&cdp_target).await;

        // Reset fb daemon để tránh kẹt trạng thái tab hoặc socket zombie cho các lượt crawl sau
        let _ = Command::new(&self.fb_bin_path)
            .arg("stop")
            .env("FB_PROFILE", &self.profile_dir)
            .env("FB_CDP", &cdp_target)
            .output()
            .await;

        let current_count = self.crawl_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        if current_count % 12 == 0 {
            info!("Crawl counter reached {} runs. Triggering Chrome auto-recycle to clean up RAM...", current_count);
            Self::trigger_chrome_recycle().await;
        }

        let output = match output_res {
            Ok(out) => out,
            Err(e) => {
                error!("Failed to launch facebook-cli binary at {}: {}", self.fb_bin_path, e);
                return Err(format!("Cannot execute facebook-cli: {}", e).into());
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout);

        if !output.status.success() && stdout.trim().is_empty() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("facebook-cli stderr: {}", stderr);
            return Err(format!("facebook-cli exited with error: {}", stderr).into());
        }
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
            } else if ref_id.is_empty() || ref_id == "-" || ref_id.starts_with("photo") {
                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(author.as_bytes());
                hasher.update(text.as_bytes());
                let hash = format!("{:x}", hasher.finalize());
                format!("anon_{}", &hash[..12])
            } else {
                ref_id.trim_matches('/').replace('/', "_")
            };

            let canonical_url = if clean_post_id.starts_with("anon_") || clean_post_id.contains('?') || clean_post_id.contains('&') {
                format!("https://www.facebook.com/groups/{}/", group_id)
            } else if !url.is_empty() && url.starts_with("http") && !url.contains("/posts/photo") && !url.contains("set=gm.") {
                url.to_string()
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

        let output = cmd.output().await;
        Self::cleanup_cdp_tabs(&cdp_target).await;
        let output = output?;
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            Ok(!stdout.contains("LOGIN_REQUIRED") && !stdout.contains("CHECKPOINT") && !stdout.contains("not logged in"))
        } else {
            Ok(false)
        }
    }
}
