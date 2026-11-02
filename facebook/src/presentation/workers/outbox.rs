use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::info;

use crate::application::use_cases::DispatchOutboxUseCase;

pub async fn run_outbox_worker(
    dispatch_use_case: Arc<DispatchOutboxUseCase>,
    notify: Option<Arc<tokio::sync::Notify>>,
) {
    info!("Outbox worker started for Anthill CRM webhook dispatching");

    loop {
        let processed = dispatch_use_case.execute_batch(10).await;
        if processed == 0 {
            // Không có event nào chờ -> đợi tín hiệu notify hoặc timeout 3s
            if let Some(ref n) = notify {
                tokio::select! {
                    _ = n.notified() => {},
                    _ = sleep(Duration::from_secs(3)) => {},
                }
            } else {
                sleep(Duration::from_secs(3)).await;
            }
        } else {
            sleep(Duration::from_millis(50)).await;
        }
    }
}
