use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub server_port: u16,
    pub server_host: String,
    pub fb_bin_path: String,
    pub chrome_profile_dir: String,
    pub groq_api_key: String,
    pub groq_primary_model: String,
    pub groq_fallback_model: String,
    pub groq_qwen_model: String,
    pub gemini_api_key: String,
    pub gemini_model: String,
    pub to_kien_webhook_url: String,
    pub to_kien_token: String,
    pub crawl_interval_seconds: i32,
    pub max_posts_per_crawl: usize,
    pub fb_cdp: String,
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
            groq_api_key: env::var("GROQ_API_KEY").unwrap_or_else(|_| "disabled".to_string()),
            groq_primary_model: env::var("GROQ_PRIMARY_MODEL")
                .unwrap_or_else(|_| "openai/gpt-oss-120b".to_string()),
            groq_fallback_model: env::var("GROQ_FALLBACK_MODEL")
                .unwrap_or_else(|_| "openai/gpt-oss-20b".to_string()),
            groq_qwen_model: env::var("GROQ_QWEN_MODEL")
                .unwrap_or_else(|_| "qwen/qwen3.6-27b".to_string()),
            gemini_api_key: env::var("GEMINI_API_KEY").unwrap_or_else(|_| "disabled".to_string()),
            gemini_model: env::var("GEMINI_MODEL")
                .unwrap_or_else(|_| "gemini-1.5-flash".to_string()),
            to_kien_webhook_url: env::var("TO_KIEN_WEBHOOK_URL")
                .unwrap_or_else(|_| "http://localhost:8080/api/v1/webhooks/social-leads".to_string()),
            to_kien_token: env::var("TO_KIEN_TOKEN").unwrap_or_else(|_| "secret_token".to_string()),
            crawl_interval_seconds: env::var("CRAWL_INTERVAL_SECONDS")
                .ok()
                .and_then(|i| i.parse().ok())
                .unwrap_or(180),
            max_posts_per_crawl: env::var("MAX_POSTS_PER_CRAWL")
                .ok()
                .and_then(|i| i.parse().ok())
                .unwrap_or(5),
            fb_cdp: env::var("FB_CDP").unwrap_or_else(|_| "9222".to_string()),
        }
    }
}
