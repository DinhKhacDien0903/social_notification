use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::info;

use crate::application::use_cases::ClassifyJobsUseCase;

pub async fn run_pipeline_worker(
    classify_use_case: Arc<ClassifyJobsUseCase>,
    notify: Option<Arc<tokio::sync::Notify>>,
) {
    info!("Pipeline worker started for classification tasks");

    loop {
        let processed = classify_use_case.execute_batch(10).await;
        if processed == 0 {
            // Không có job nào chờ -> đợi tín hiệu notify hoặc timeout 3s
            if let Some(ref n) = notify {
                tokio::select! {
                    _ = n.notified() => {},
                    _ = sleep(Duration::from_secs(3)) => {},
                }
            } else {
                sleep(Duration::from_secs(3)).await;
            }
        } else {
            // Có job vừa xử lý xong -> chạy ngay đợt tiếp để drain nhanh
            sleep(Duration::from_millis(50)).await;
        }
    }
}
