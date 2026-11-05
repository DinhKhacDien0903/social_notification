use std::sync::Arc;
use chrono::{DateTime, Duration, Utc};
use rand::Rng;
use sha2::{Digest, Sha256};
use tracing::{error, info, warn};
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

use crate::application::interfaces::{AnthillLeadPayload, FacebookEngine};
use crate::domain::{
    JobRepository, NewCrawlRun, NewJob, NewOutboxEvent, NewPost, OutboxRepository, Post,
    PostRepository, SessionRepository, TargetRepository, WatchTarget,
};

pub struct CrawlTargetUseCase {
    engine: Arc<dyn FacebookEngine>,
    target_repo: Arc<dyn TargetRepository>,
    post_repo: Arc<dyn PostRepository>,
    job_repo: Arc<dyn JobRepository>,
    session_repo: Arc<dyn SessionRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    max_posts: usize,
    pipeline_notify: Option<Arc<tokio::sync::Notify>>,
    crawl_lock: tokio::sync::Mutex<()>,
}

impl CrawlTargetUseCase {
    pub fn new(
        engine: Arc<dyn FacebookEngine>,
        target_repo: Arc<dyn TargetRepository>,
        post_repo: Arc<dyn PostRepository>,
        job_repo: Arc<dyn JobRepository>,
        session_repo: Arc<dyn SessionRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        max_posts: usize,
        pipeline_notify: Option<Arc<tokio::sync::Notify>>,
    ) -> Self {
        Self {
            engine,
            target_repo,
            post_repo,
            job_repo,
            session_repo,
            outbox_repo,
            max_posts,
            pipeline_notify,
            crawl_lock: tokio::sync::Mutex::new(()),
        }
    }

    /// Kiểm tra xem thông điệp lỗi có phải là lỗi xác thực / login / cookie / session hay không.
    pub fn is_auth_error(err_str: &str) -> bool {
        let lower = err_str.to_lowercase();
        lower.contains("not logged in")
            || lower.contains("login_required")
            || lower.contains("checkpoint")
            || lower.contains("fb login")
            || lower.contains("login.php")
            || lower.contains("/login")
            || lower.contains("session expired")
            || lower.contains("invalid session")
            || lower.contains("session not initialized")
            || (lower.contains("cookie") && (lower.contains("missing") || lower.contains("invalid") || lower.contains("expired") || lower.contains("error")))
            || lower.contains("c_user")
            || lower.contains("unauthorized")
            || lower.contains("401 unauthorized")
            || lower.contains("403 forbidden")
    }

    /// Tính toán next_due_at thông minh chống dồn dập (Anti-Clustering & Staggering Engine):
    /// 1. Base interval kèm jitter ±5-6%.
    /// 2. Giãn cách Night Mode (23:00 -> 06:00 VN: x2.5).
    /// 3. Collision Avoidance: Đảm bảo khoảng cách tối thiểu MIN_GAP_SECONDS (90 giây)
    ///    với mọi nhóm đang bật khác, tránh tình trạng quét đồng thời dồn dập tại cùng 1 phút.
    pub async fn calculate_staggered_next_due(
        target_repo: &Arc<dyn TargetRepository>,
        target_id: i64,
        base_interval_seconds: i32,
    ) -> DateTime<Utc> {
        let jitter_percent: f64 = {
            let mut rng = rand::thread_rng();
            rng.gen_range(-0.06..0.06)
        };
        let base = base_interval_seconds.max(300) as f64;

        // Giờ Việt Nam (UTC + 7)
        let vn_hour = (Utc::now() + Duration::hours(7))
            .format("%H")
            .to_string()
            .parse::<u32>()
            .unwrap_or(12);

        // Khung giờ đêm (23:00 -> 06:00): Tự động giãn chu kỳ gấp 2.5 lần
        let multiplier = if vn_hour >= 23 || vn_hour < 6 {
            2.5
        } else {
            1.0
        };

        const MIN_GAP_SECONDS: i64 = 120;
        let interval_secs = (base * multiplier * (1.0 + jitter_percent)).round() as i64;
        let mut candidate = Utc::now() + Duration::seconds(interval_secs.max(MIN_GAP_SECONDS));

        if let Ok(other_times) = target_repo.get_active_scheduled_times(target_id).await {
            let mut sorted_times = other_times;
            sorted_times.sort();

            let mut collision = true;
            let mut attempts = 0;
            while collision && attempts < 30 {
                collision = false;
                attempts += 1;
                for t in &sorted_times {
                    let diff = (candidate.timestamp() - t.timestamp()).abs();
                    if diff < MIN_GAP_SECONDS {
                        // Tránh va chạm: dịch candidate ra sau mốc va chạm + MIN_GAP_SECONDS
                        candidate = *t + Duration::seconds(MIN_GAP_SECONDS);
                        collision = true;
                        break;
                    }
                }
            }
        }

        candidate
    }

    /// Tính content_hash chuẩn hóa NFC bằng SHA-256
    pub fn compute_content_hash(text: &str) -> String {
        let nfc_text: String = text.nfc().collect();
        let cleaned = nfc_text.split_whitespace().collect::<Vec<_>>().join(" ");
        let mut hasher = Sha256::new();
        hasher.update(cleaned.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    pub async fn execute(&self, target: &WatchTarget) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let _crawl_guard = self.crawl_lock.lock().await;

        // 1. Kiểm tra trạng thái phiên trước khi crawl: CHỈ CRAWL KHI PHIÊN CÒN HOẠT ĐỘNG
        let is_ready = self.session_repo.is_session_ready(&target.session_id).await.unwrap_or(false);
        if !is_ready {
            warn!(
                "Skipping crawl for target group '{}' ({}) because session '{}' is not READY. Crawling is suspended.",
                target.display_name, target.group_id, target.session_id
            );
            return Err("Session is not active or ready. Crawling is suspended.".into());
        }

        let run_id = Uuid::new_v4().to_string();
        info!("Starting crawl run {} for target group '{}' ({})", run_id, target.display_name, target.group_id);

        let posts_result = self.engine.fetch_group_posts(&target.group_id, self.max_posts).await;

        let raw_posts = match posts_result {
            Ok(p) => {
                // Khi crawl thành công, reset bộ đếm lỗi auth toàn cục của session về 0
                let _ = self.session_repo.reset_auth_failure(&target.session_id).await;
                p
            }
            Err(e) => {
                let err_msg = e.to_string();
                error!("Failed to fetch posts for group {}: {}", target.group_id, err_msg);

                // Kiểm tra lỗi có liên quan tới Auth / Login / Session / Cookie hay không
                if Self::is_auth_error(&err_msg) {
                    let auth_fail_count = self.session_repo.increment_auth_failure(&target.session_id).await.unwrap_or(2);
                    warn!(
                        "Auth issue detected on group '{}' ({}) for session '{}'. Global auth failure count: {}/2",
                        target.display_name, target.group_id, target.session_id, auth_fail_count
                    );

                    // NGUYÊN TẮC: Khi gặp lỗi liên quan tới auth => chỉ cần crawl tới lần 2 bất kể cùng hoặc khác group
                    // => push notification về admin và ngừng crawl!
                    if auth_fail_count >= 2 {
                        error!(
                            "CIRCUIT BREAKER TRIGGERED: Session '{}' reached 2 consecutive auth failures across groups. Halting all crawl runs and alerting Admin!",
                            target.session_id
                        );

                        // 1. Cập nhật trạng thái session sang LOGIN_REQUIRED hoặc CHECKPOINT_REQUIRED
                        let status_val = if err_msg.to_lowercase().contains("checkpoint") {
                            "CHECKPOINT_REQUIRED"
                        } else {
                            "LOGIN_REQUIRED"
                        };
                        let _ = self.session_repo.update_status(
                            &target.session_id,
                            status_val,
                            Some("AUTH_FAILURE_LIMIT_REACHED"),
                        ).await;

                        // 2. Bắn thông báo đẩy (Push Notification) về Admin qua Outbox
                        let alert_payload = AnthillLeadPayload {
                            event_key: format!("session_auth_failure_{}_{}", target.session_id, Utc::now().timestamp()),
                            event_type: Some("social_session_expired".to_string()),
                            title: Some("⚠️ Cảnh báo: Phiên Facebook đã hết hạn!".to_string()),
                            body: Some(format!(
                                "Phát hiện lỗi xác thực Facebook (2 lần lỗi liên tiếp). Hệ thống đã tự động dừng crawl để bảo vệ nick. Vui lòng nạp lại session cookies trên trang Admin."
                            )),
                            target_url: Some("/admin".to_string()),
                            platform: target.platform.clone(),
                            group_id: target.group_id.clone(),
                            post_id: "session_alert".to_string(),
                            author: "System Monitor".to_string(),
                            canonical_url: format!("https://www.facebook.com/groups/{}/", target.group_id),
                            content: format!("Lỗi xác thực Facebook: {}", err_msg),
                            intent: "session_alert".to_string(),
                            confidence: 1.0,
                            location: None,
                            requested_date: None,
                            summary: "Phiên đăng nhập đã hết hạn hoặc cookie không hợp lệ sau 2 lần thử lại liên tiếp.".to_string(),
                            created_at: Utc::now().to_rfc3339(),
                        };

                        if let Ok(payload_json) = serde_json::to_string(&alert_payload) {
                            let _ = self.outbox_repo.insert_event(NewOutboxEvent {
                                event_key: alert_payload.event_key.clone(),
                                payload: payload_json,
                            }).await;
                            info!("Successfully created Outbox alert event for Admin push notification");
                        }
                    }
                }

                // Quick Retry cho lần lỗi đầu tiên (120s), lần hai (300s),
                // và Exponential Backoff từ lần 3 trở đi để bảo vệ tài khoản khỏi checkpoint
                let retry_delay_seconds = match target.failure_count {
                    0 => 120, // Thử lại nhanh sau 2 phút để không bỏ sót lead
                    1 => 300, // Thử lại sau 5 phút
                    fc => (target.interval_seconds as i64) * 2_i64.pow(((fc - 1) as u32).min(2)),
                };
                let mut next_due = Utc::now() + Duration::seconds(retry_delay_seconds);
                // Đảm bảo retry không va chạm (< 60s) với lịch quét của các nhóm khác
                if let Ok(other_times) = self.target_repo.get_active_scheduled_times(target.id).await {
                    for t in other_times {
                        if (next_due.timestamp() - t.timestamp()).abs() < 60 {
                            next_due = t + Duration::seconds(90);
                        }
                    }
                }
                let _ = self.target_repo.update_target_failure(target.id, next_due).await;

                let _ = self.target_repo.record_crawl_run(NewCrawlRun {
                    run_id,
                    target_id: target.id,
                    status: "ERROR".to_string(),
                    observed_count: 0,
                    new_count: 0,
                    stop_reason: Some("FETCH_FAILED".to_string()),
                    coverage: "none".to_string(),
                    error_code: Some(err_msg),
                    parser_version: "social-listener-rust-v3.0.0".to_string(),
                }).await;

                return Err(e);
            }
        };

        let observed_count = raw_posts.len() as i32;
        let mut new_count = 0;
        let mut consecutive_old_posts = 0;
        let mut stop_reason = "COMPLETED_SCAN";
        let mut newest_watermark = target.watermark;

        for raw in raw_posts {
            // Overlap Window: Nếu gặp liên tiếp 3 bài cũ không thay đổi -> Dừng lượt quét sớm để tiết kiệm tài nguyên
            if consecutive_old_posts >= 2 {
                info!("Encountered 2 consecutive unchanged posts. Stopping early due to overlap window.");
                stop_reason = "STOP_2_OLD_POSTS";
                break;
            }

            // Nếu bài viết đã cũ hơn hoặc bằng watermark đã biết -> tăng biến đếm bài cũ
            if let Some(watermark) = target.watermark {
                if let Some(src_time) = raw.source_created_at {
                    if src_time <= watermark {
                        consecutive_old_posts += 1;
                        if consecutive_old_posts >= 2 {
                            info!("Reached watermark timestamp. Stopping early due to watermark check.");
                            stop_reason = "STOP_WATERMARK";
                            break;
                        }
                    }
                }
            }

            let identity_key = Post::compute_identity_key(&target.platform, &target.group_id, &raw.post_id);
            let content_hash = Self::compute_content_hash(&raw.text);

            let new_post = NewPost {
                platform: target.platform.clone(),
                group_id: target.group_id.clone(),
                post_id: raw.post_id.clone(),
                canonical_url: raw.canonical_url.clone(),
                identity_key,
                content_hash,
                text: raw.text,
                text_complete: raw.text_complete,
                author_display: raw.author_display,
                source_created_at: raw.source_created_at,
            };

            match self.post_repo.upsert_post(new_post).await {
                Ok((post, is_new, is_updated)) => {
                    if let Some(src_time) = post.source_created_at {
                        if newest_watermark.map_or(true, |w| src_time > w) {
                            newest_watermark = Some(src_time);
                        }
                    }

                    if is_new {
                        new_count += 1;
                        consecutive_old_posts = 0;
                        // Sinh Job CLASSIFY
                        let _ = self.job_repo.create_job(NewJob {
                            kind: "CLASSIFY".to_string(),
                            post_id: post.id,
                            revision: post.revision,
                        }).await;
                        // Kích hoạt pipeline phân loại ngay lập tức (Instant Pipeline Activation)
                        if let Some(ref notify) = self.pipeline_notify {
                            notify.notify_one();
                        }
                    } else if is_updated {
                        consecutive_old_posts = 0;
                        info!("Post {} updated revision to {}, creating new CLASSIFY job", post.id, post.revision);
                        let _ = self.job_repo.create_job(NewJob {
                            kind: "CLASSIFY".to_string(),
                            post_id: post.id,
                            revision: post.revision,
                        }).await;
                        if let Some(ref notify) = self.pipeline_notify {
                            notify.notify_one();
                        }
                    } else {
                        consecutive_old_posts += 1;
                        let _ = self.post_repo.touch_last_seen(post.id).await;
                    }
                }
                Err(e) => {
                    warn!("Failed to upsert post: {}", e);
                }
            }
        }

        // Tính toán next_due_at thông minh: tự động phân bổ lịch giãn cách, chống va chạm (Anti-Clustering)
        let next_due = Self::calculate_staggered_next_due(&self.target_repo, target.id, target.interval_seconds).await;

        let _ = self.target_repo.update_target_success(target.id, newest_watermark, next_due).await;

        let _ = self.target_repo.record_crawl_run(NewCrawlRun {
            run_id: run_id.clone(),
            target_id: target.id,
            status: "SUCCESS".to_string(),
            observed_count,
            new_count,
            stop_reason: Some(stop_reason.to_string()),
            coverage: "incremental".to_string(),
            error_code: None,
            parser_version: "social-listener-rust-v3.0.0".to_string(),
        }).await;

        info!(
            "Finished crawl run {}: observed={}, new={}, stop_reason={}",
            run_id, observed_count, new_count, stop_reason
        );

        Ok(())
    }
}
