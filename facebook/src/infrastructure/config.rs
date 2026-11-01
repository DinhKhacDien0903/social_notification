use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub server_port: u16,
    pub server_host: String,
    pub fb_bin_path: String,
    pub chrome_profile_dir: String,
    pub gemini_api_key: String,
    pub to_kien_webhook_url: String,
    pub to_kien_token: String,
    pub crawl_interval_seconds: i32,
}

impl AppConfig {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        Self {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite://social_listener.db?mode=rwc".to_string()),
            server_port: env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8000),
            server_host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            fb_bin_path: env::var("FB_BIN_PATH").unwrap_or_else(|_| "./bin/fb".to_string()),
            chrome_profile_dir: env::var("CHROME_PROFILE_DIR")
                .unwrap_or_else(|_| "./runtime/profile".to_string()),
            gemini_api_key: env::var("GEMINI_API_KEY").unwrap_or_else(|_| "disabled".to_string()),
            to_kien_webhook_url: env::var("TO_KIEN_WEBHOOK_URL")
                .unwrap_or_else(|_| "http://localhost:5000/api/v1/webhooks/social-leads".to_string()),
            to_kien_token: env::var("TO_KIEN_TOKEN").unwrap_or_else(|_| "secret_token".to_string()),
            crawl_interval_seconds: env::var("CRAWL_INTERVAL_SECONDS")
                .ok()
                .and_then(|i| i.parse().ok())
                .unwrap_or(300),
        }
    }
}
