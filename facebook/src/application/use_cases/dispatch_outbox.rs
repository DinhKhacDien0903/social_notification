use std::sync::Arc;
use tracing::{error, info, warn};

use crate::application::interfaces::{AnthillLeadPayload, AnthillNotifier};
use crate::domain::OutboxRepository;

pub struct DispatchOutboxUseCase {
    outbox_repo: Arc<dyn OutboxRepository>,
    notifier: Arc<dyn AnthillNotifier>,
    worker_id: String,
}

impl DispatchOutboxUseCase {
    pub fn new(
        outbox_repo: Arc<dyn OutboxRepository>,
        notifier: Arc<dyn AnthillNotifier>,
        worker_id: String,
    ) -> Self {
        Self {
            outbox_repo,
            notifier,
            worker_id,
        }
    }

    pub async fn execute_batch(&self, limit: i64) -> usize {
        let events = match self.outbox_repo.claim_pending_events(&self.worker_id, limit, 60).await {
            Ok(evs) => evs,
            Err(e) => {
                error!("Failed to claim pending outbox events: {}", e);
                return 0;
            }
        };

        let count = events.len();
        if count == 0 {
            return 0;
        }

        info!("Claimed {} outbox events for dispatch", count);

        for ev in events {
            let payload: Result<AnthillLeadPayload, _> = serde_json::from_str(&ev.payload);
            let payload = match payload {
                Ok(p) => p,
                Err(err) => {
                    error!("Corrupted JSON payload in outbox event {}: {}", ev.id, err);
                    let _ = self.outbox_repo.mark_dead_letter(ev.id, &format!("Invalid JSON: {}", err)).await;
                    continue;
                }
            };

            match self.notifier.send_lead(&ev.event_key, &payload).await {
                Ok(remote_id) => {
                    info!("Successfully delivered outbox event {} (key: {})", ev.id, ev.event_key);
                    let _ = self.outbox_repo.mark_delivered(ev.id, remote_id.as_deref()).await;
                }
                Err(err) => {
                    let err_msg = err.to_string();
                    warn!("Failed to dispatch outbox event {}: {}", ev.id, err_msg);

                    if err_msg.contains("401") || err_msg.contains("403") {
                        // Lỗi xác thực token không thể retry tự khỏi
                        let _ = self.outbox_repo.mark_dead_letter(ev.id, &err_msg).await;
                    } else if ev.attempts >= 4 {
                        // Đã thử 5 lần (0, 1, 2, 3, 4) -> chuyển sang DEAD_LETTER
                        let _ = self.outbox_repo.mark_dead_letter(ev.id, &format!("Max attempts exceeded: {}", err_msg)).await;
                    } else {
                        // Exponential backoff: 5s, 10s, 20s, 40s
                        let delay_secs = 5 * (1 << ev.attempts);
                        let _ = self.outbox_repo.mark_retry(ev.id, delay_secs, &err_msg).await;
                    }
                }
            }
        }

        count
    }
}
