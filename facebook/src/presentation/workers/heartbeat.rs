use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{debug, info};

use crate::domain::SessionRepository;

/// Background worker duy trì phiên hoạt động (Presence Heartbeat) cho tài khoản Facebook
/// Được thiết kế 100% Native Rust Async, giao tiếp trực tiếp qua Chrome CDP HTTP API,
/// không spawn tiến trình Python con định kỳ -> Giảm thiểu 100% CPU/RAM dư thừa.
pub async fn run_heartbeat_worker(session_repo: Arc<dyn SessionRepository>) {
    info!("Native Rust Session Heartbeat worker started (interval: 300s)");

    // Nghỉ 30s sau khi khởi động để Chrome và các tiến trình ban đầu ổn định
    sleep(Duration::from_secs(30)).await;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let cdp_port = std::env::var("FB_CDP").unwrap_or_else(|_| "9222".to_string());
    let port = if let Some(pos) = cdp_port.find("127.0.0.1:") {
        let after = &cdp_port[pos + "127.0.0.1:".len()..];
        after.split('/').next().unwrap_or("9222")
    } else if cdp_port.chars().all(|c| c.is_ascii_digit()) {
        cdp_port.as_str()
    } else {
        cdp_port.trim_start_matches(':')
    };

    let host_header = format!("127.0.0.1:{}", port);
    let list_url = format!("http://127.0.0.1:{}/json/list", port);

    loop {
        let is_ready = session_repo.is_session_ready("primary").await.unwrap_or(false);
        if is_ready {
            match client.get(&list_url).header("Host", &host_header).send().await {
                Ok(resp) => {
                    if let Ok(pages) = resp.json::<Vec<serde_json::Value>>().await {
                        // 1. Tìm anchor tab Facebook chính để ping duy trì Comet
                        let fb_page = pages.iter().find(|p| {
                            let tab_type = p.get("type").and_then(|t| t.as_str()).unwrap_or("");
                            let tab_url = p.get("url").and_then(|u| u.as_str()).unwrap_or("");
                            tab_type == "page" && tab_url.contains("facebook.com")
                        });

                        if let Some(p) = fb_page {
                            if let Some(target_id) = p.get("id").and_then(|i| i.as_str()) {
                                // Gọi /json/activate/{id} để focus tab, kích hoạt native document.visibilityState = visible
                                // và giữ kết nối Comet MQTT / WebSocket của Facebook luôn active 24/7
                                let activate_url = format!("http://127.0.0.1:{}/json/activate/{}", port, target_id);
                                let _ = client.get(&activate_url).header("Host", &host_header).send().await;
                                debug!("[Heartbeat] Native ping: Activated Facebook Comet anchor tab ({})", target_id);
                            }
                        } else {
                            // 2. Nếu anchor tab bị thiếu hoặc vô tình bị đóng, tự động tạo lại anchor tab Facebook
                            info!("[Heartbeat] Facebook anchor tab missing in Chrome. Re-opening persistent anchor...");
                            let new_tab_url = format!("http://127.0.0.1:{}/json/new?https://www.facebook.com/", port);
                            if let Ok(new_resp) = client.put(&new_tab_url).header("Host", &host_header).send().await {
                                if let Ok(json) = new_resp.json::<serde_json::Value>().await {
                                    info!("[Heartbeat] Successfully re-created Facebook Comet anchor tab ({:?})", json.get("id"));
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    debug!("[Heartbeat] Chrome CDP on port {} temporarily unavailable: {}", port, e);
                }
            }
        }

        // Tối ưu chu kỳ: Giãn lên 300s (5 phút) thay vì 180s, giảm tần suất đánh thức, tiết kiệm tối đa CPU
        sleep(Duration::from_secs(300)).await;
    }
}
 