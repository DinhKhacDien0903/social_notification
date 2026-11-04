use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{error, info};

use crate::application::use_cases::CrawlTargetUseCase;
use crate::domain::{SessionRepository, TargetRepository};

pub async fn run_scheduler_worker(
    target_repo: Arc<dyn TargetRepository>,
    session_repo: Arc<dyn SessionRepository>,
    crawl_use_case: Arc<CrawlTargetUseCase>,
    worker_id: String,
) {
    info!("Scheduler worker started with worker_id: {}", worker_id);

    loop {
        // KIỂM TRA ĐIỀU KIỆN CRAWL:
        // Crawl CHỈ chạy khi session còn hoạt động (status = READY và auth_failure_count < 2).
        // Khi gặp lỗi auth tới lần 2 -> session status chuyển thành LOGIN_REQUIRED -> scheduler tự động ngừng crawl.
        // Khi người dùng import session thành công và session hợp lệ -> status = READY -> scheduler tự động tiếp tục crawl.
        let is_ready = session_repo.is_session_ready("primary").await.unwrap_or(false);
        if !is_ready {
            info!("[Scheduler] Crawling is PAUSED: primary session is not READY. Waiting for valid session import...");
            sleep(Duration::from_secs(20)).await;
            continue;
        }

        // Chỉ claim 1 target duy nhất mỗi chu kỳ để triệt tiêu tình trạng dồn dập nhiều nhóm cùng lúc
        match target_repo.claim_due_targets(&worker_id, 1).await {
            Ok(targets) => {
                for target in targets {
                    info!("Target '{}' (group: {}) is due. Executing crawl...", target.display_name, target.group_id);
                    if let Err(e) = crawl_use_case.execute(&target).await {
                        error!("Crawl failed for group {}: {}", target.group_id, e);
                    }

                    // COOLING PERIOD (Làm mát giữa các lượt quét):
                    // Bắt buộc nghỉ 20s để Chrome dọn dẹp tiến trình DOM, CPU hạ về 0%
                    // trước khi tiến hành quét nhóm tiếp theo.
                    info!("Crawl completed. Cooling down for 20s to prevent CPU & RAM spikes...");
                    sleep(Duration::from_secs(20)).await;
                }
            }
            Err(e) => {
                error!("Error querying due targets: {}", e);
            }
        }

        sleep(Duration::from_secs(10)).await;
    }
}
