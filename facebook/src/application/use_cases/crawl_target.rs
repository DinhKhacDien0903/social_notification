use std::sync::Arc;
use chrono::{Duration, Utc};
use rand::Rng;
use sha2::{Digest, Sha256};
use tracing::{error, info, warn};
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

use crate::application::interfaces::FacebookEngine;
use crate::domain::{
    JobRepository, NewCrawlRun, NewJob, NewPost, Post, PostRepository, TargetRepository,
    WatchTarget,
};

pub struct CrawlTargetUseCase {
    engine: Arc<dyn FacebookEngine>,
    target_repo: Arc<dyn TargetRepository>,
    post_repo: Arc<dyn PostRepository>,
    job_repo: Arc<dyn JobRepository>,
}

impl CrawlTargetUseCase {
    pub fn new(
        engine: Arc<dyn FacebookEngine>,
        target_repo: Arc<dyn TargetRepository>,
        post_repo: Arc<dyn PostRepository>,
        job_repo: Arc<dyn JobRepository>,
    ) -> Self {
        Self {
            engine,
            target_repo,
            post_repo,
            job_repo,
        }
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
        let run_id = Uuid::new_v4().to_string();
        info!("Starting crawl run {} for target group '{}' ({})", run_id, target.display_name, target.group_id);

        let posts_result = self.engine.fetch_group_posts(&target.group_id, 25).await;

        let raw_posts = match posts_result {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to fetch posts for group {}: {}", target.group_id, e);
                // Cập nhật failure_count và next_due_at lùi lại
                let next_due = Utc::now() + Duration::seconds(target.interval_seconds as i64);
                let _ = self.target_repo.update_target_failure(target.id, next_due).await;

                let _ = self.target_repo.record_crawl_run(NewCrawlRun {
                    run_id,
                    target_id: target.id,
                    status: "ERROR".to_string(),
                    observed_count: 0,
                    new_count: 0,
                    stop_reason: Some("FETCH_FAILED".to_string()),
                    coverage: "none".to_string(),
                    error_code: Some(e.to_string()),
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
            // Overlap Window: Nếu gặp liên tiếp 5 bài cũ không thay đổi -> Dừng lượt quét sớm để tiết kiệm tài nguyên
            if consecutive_old_posts >= 5 {
                info!("Encountered 5 consecutive unchanged posts. Stopping early due to overlap window.");
                stop_reason = "STOP_5_OLD_POSTS";
                break;
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
                    } else if is_updated {
                        consecutive_old_posts = 0;
                        info!("Post {} updated revision to {}, creating new CLASSIFY job", post.id, post.revision);
                        let _ = self.job_repo.create_job(NewJob {
                            kind: "CLASSIFY".to_string(),
                            post_id: post.id,
                            revision: post.revision,
                        }).await;
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

        // Tính Jitter ±5% cho next_due_at (đóng scope rng trước await để thỏa mãn Send)
        let interval_with_jitter = {
            let mut rng = rand::thread_rng();
            let jitter_percent: f64 = rng.gen_range(-0.05..0.05);
            (target.interval_seconds as f64 * (1.0 + jitter_percent)).round() as i64
        };
        let next_due = Utc::now() + Duration::seconds(interval_with_jitter);

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
