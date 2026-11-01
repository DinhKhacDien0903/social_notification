use std::sync::Arc;
use tracing::{error, info, warn};

use crate::application::classifier::ClassifierService;
use crate::application::interfaces::AnthillLeadPayload;
use crate::domain::{JobRepository, LeadRepository, NewOutboxEvent, OutboxRepository, PostRepository};

pub struct ClassifyJobsUseCase {
    post_repo: Arc<dyn PostRepository>,
    job_repo: Arc<dyn JobRepository>,
    lead_repo: Arc<dyn LeadRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    classifier_service: Arc<ClassifierService>,
    worker_id: String,
}

impl ClassifyJobsUseCase {
    pub fn new(
        post_repo: Arc<dyn PostRepository>,
        job_repo: Arc<dyn JobRepository>,
        lead_repo: Arc<dyn LeadRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        classifier_service: Arc<ClassifierService>,
        worker_id: String,
    ) -> Self {
        Self {
            post_repo,
            job_repo,
            lead_repo,
            outbox_repo,
            classifier_service,
            worker_id,
        }
    }

    pub async fn execute_batch(&self, limit: i64) -> usize {
        let jobs = match self.job_repo.claim_pending_jobs(&self.worker_id, limit, 60).await {
            Ok(j) => j,
            Err(e) => {
                error!("Failed to claim pending jobs: {}", e);
                return 0;
            }
        };

        let count = jobs.len();
        if count == 0 {
            return 0;
        }

        info!("Claimed {} jobs for classification", count);

        for job in jobs {
            let post_opt = match self.post_repo.find_by_id(job.post_id).await {
                Ok(p) => p,
                Err(e) => {
                    warn!("Failed to fetch post {}: {}", job.post_id, e);
                    let _ = self.job_repo.fail_job(job.id, 60).await;
                    continue;
                }
            };

            let post = match post_opt {
                Some(p) => p,
                None => {
                    warn!("Post {} not found, completing orphan job {}", job.post_id, job.id);
                    let _ = self.job_repo.complete_job(job.id).await;
                    continue;
                }
            };

            let result = self.classifier_service.evaluate_post(post.id, &post.text).await;

            if result.is_lead {
                if let Some(new_lead) = result.lead {
                    match self.lead_repo.create_lead(new_lead.clone()).await {
                        Ok(lead) => {
                            info!("Saved Lead ID {} for Post {}", lead.id, post.id);

                            // Tạo Transactional Outbox Event
                            let event_key = format!(
                                "{}:{}:{}:lead:{}:team-sales",
                                post.platform, post.group_id, post.post_id, lead.rule_version
                            );

                            let payload = AnthillLeadPayload {
                                event_key: event_key.clone(),
                                platform: post.platform.clone(),
                                group_id: post.group_id.clone(),
                                post_id: post.post_id.clone(),
                                author: post.author_display.clone(),
                                canonical_url: post.canonical_url.clone(),
                                content: post.text.clone(),
                                intent: lead.intent.clone(),
                                confidence: lead.confidence,
                                location: lead.location.clone(),
                                requested_date: lead.requested_date.clone(),
                                summary: lead.summary.clone(),
                                created_at: chrono::Utc::now().to_rfc3339(),
                            };

                            if let Ok(json_str) = serde_json::to_string(&payload) {
                                let _ = self.outbox_repo.insert_event(NewOutboxEvent {
                                    event_key,
                                    payload: json_str,
                                }).await;
                            }
                        }
                        Err(e) => {
                            error!("Failed to save Lead for post {}: {}", post.id, e);
                        }
                    }
                }
            }

            let _ = self.job_repo.complete_job(job.id).await;
        }

        count
    }
}
