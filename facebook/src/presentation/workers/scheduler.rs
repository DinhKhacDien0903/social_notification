use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{error, info};

use crate::application::use_cases::CrawlTargetUseCase;
use crate::domain::TargetRepository;

pub async fn run_scheduler_worker(
    target_repo: Arc<dyn TargetRepository>,
    crawl_use_case: Arc<CrawlTargetUseCase>,
    worker_id: String,
) {
    info!("Scheduler worker started with worker_id: {}", worker_id);

    loop {
        match target_repo.claim_due_targets(&worker_id, 3).await {
            Ok(targets) => {
                for target in targets {
                    info!("Target '{}' (group: {}) is due. Executing crawl...", target.display_name, target.group_id);
                    let uc = Arc::clone(&crawl_use_case);
                    tokio::spawn(async move {
                        if let Err(e) = uc.execute(&target).await {
                            error!("Crawl failed for group {}: {}", target.group_id, e);
                        }
                    });
                }
            }
            Err(e) => {
                error!("Error querying due targets: {}", e);
            }
        }

        sleep(Duration::from_secs(10)).await;
    }
}
