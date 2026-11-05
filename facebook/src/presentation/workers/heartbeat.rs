use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{debug, info, warn};

use crate::domain::SessionRepository;

pub async fn run_heartbeat_worker(session_repo: Arc<dyn SessionRepository>) {
    info!("Session Heartbeat worker started (interval: 180s)");

    // Delay 30s after startup to let Chrome & initial crawls settle
    sleep(Duration::from_secs(30)).await;

    loop {
        let is_ready = session_repo.is_session_ready("primary").await.unwrap_or(false);
        if is_ready {
            let script = "/app/bin/inject_cookies.py";
            if std::path::Path::new(script).exists() {
                match tokio::process::Command::new("python3")
                    .arg(script)
                    .arg("--heartbeat")
                    .output()
                    .await
                {
                    Ok(out) if out.status.success() => {
                        debug!("[Heartbeat] Facebook presence kept alive and cookies verified.");
                    }
                    Ok(out) => {
                        let err = String::from_utf8_lossy(&out.stderr);
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        warn!(
                            "[Heartbeat] Heartbeat returned non-zero ({}): {} | {}",
                            out.status,
                            err.trim(),
                            stdout.trim()
                        );
                    }
                    Err(e) => {
                        warn!("[Heartbeat] Failed to execute heartbeat script: {}", e);
                    }
                }
            }
        }

        // Run heartbeat every 3 minutes (180s)
        sleep(Duration::from_secs(180)).await;
    }
}
