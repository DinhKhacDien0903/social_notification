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

        // Chuẩn bị biến môi trường FB_PROFILE
        let mut cmd = Command::new(&self.fb_bin_path);
        cmd.arg("posts")
            .arg(&formatted_target)
            .arg("-n")
            .arg(max_posts.to_string())
            .arg("--urls")
            .arg("--full")
            .env("FB_PROFILE", &self.profile_dir)
            .env("FB_CDP", &self.fb_cdp)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = match cmd.output().await {
            Ok(out) => out,
            Err(e) => {
                error!("Failed to launch facebook-cli binary at {}: {}", self.fb_bin_path, e);
                return Err(format!("Cannot execute facebook-cli: {}", e).into());
            }
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("facebook-cli stderr: {}", stderr);
            return Err(format!("facebook-cli exited with error: {}", stderr).into());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut results = Vec::new();

        // Từng dòng TSV:
        // Cấu trúc chuẩn của `fb posts`:
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

            // Xử lý các cột từ TSV
            let (ref_id, author, _time_str, _stats, text, url) = match parts.len() {
                len if len >= 6 => (parts[0], parts[1], parts[2], parts[3], parts[4], parts[5]),
                5 => (parts[0], parts[1], parts[2], parts[3], parts[4], ""),
                4 => (parts[0], parts[1], parts[2], "", parts[3], ""),
                3 => (parts[0], parts[1], "", "", parts[2], ""),
                2 => (parts[0], "", "", "", parts[1], ""),
                _ => (parts[0], "", "", "", "", ""),
            };

            let post_id = if !url.is_empty() {
                self.extract_post_id(url).unwrap_or_else(|| ref_id.to_string())
            } else {
                ref_id.to_string()
            };

            let canonical_url = if !url.is_empty() && url.starts_with("http") {
                url.to_string()
            } else {
                format!("https://www.facebook.com/groups/{}/posts/{}/", group_id, post_id)
            };

            results.push(RawPostDto {
                post_id,
                canonical_url,
                author_display: author.to_string(),
                text: text.to_string(),
                text_complete: true,
                source_created_at: None,
            });
        }

        debug!("Extracted {} raw posts from facebook-cli", results.len());
        Ok(results)
    }

    async fn check_session_health(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let mut cmd = Command::new(&self.fb_bin_path);
        cmd.arg("status")
            .env("FB_PROFILE", &self.profile_dir)
            .env("FB_CDP", &self.fb_cdp)
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
