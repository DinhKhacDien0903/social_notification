use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::info;

use crate::application::use_cases::ClassifyJobsUseCase;

pub async fn run_pipeline_worker(classify_use_case: Arc<ClassifyJobsUseCase>) {
    info!("Pipeline worker started for classification tasks");

    loop {
        let processed = classify_use_case.execute_batch(10).await;
        if processed == 0 {
            // Không có job nào chờ -> sleep 5 giây
            sleep(Duration::from_secs(5)).await;
        } else {
            // Có job vừa xử lý xong -> chạy ngay đợt tiếp để drain nhanh
            sleep(Duration::from_millis(100)).await;
        }
    }
}
