use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::info;

use crate::application::use_cases::DispatchOutboxUseCase;

pub async fn run_outbox_worker(dispatch_use_case: Arc<DispatchOutboxUseCase>) {
    info!("Outbox worker started for Anthill CRM webhook dispatching");

    loop {
        let processed = dispatch_use_case.execute_batch(10).await;
        if processed == 0 {
            sleep(Duration::from_secs(5)).await;
        } else {
            sleep(Duration::from_millis(100)).await;
        }
    }
}
