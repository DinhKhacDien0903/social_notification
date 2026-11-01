use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::{
    lead::{Lead, NewLead},
    outbox::{Job, NewJob, NewOutboxEvent, OutboxEvent},
    post::{NewPost, Post},
    session::SocialSession,
    target::{CrawlRun, NewCrawlRun, NewWatchTarget, WatchTarget},
};

#[async_trait]
pub trait TargetRepository: Send + Sync {
    async fn get_all_targets(&self) -> Result<Vec<WatchTarget>, sqlx::Error>;
    async fn get_target_by_id(&self, id: i64) -> Result<Option<WatchTarget>, sqlx::Error>;
    async fn create_target(&self, target: NewWatchTarget) -> Result<WatchTarget, sqlx::Error>;
    async fn toggle_target(&self, id: i64, enabled: bool) -> Result<bool, sqlx::Error>;
    async fn claim_due_targets(&self, lease_owner: &str, limit: i64) -> Result<Vec<WatchTarget>, sqlx::Error>;
    async fn update_target_success(&self, id: i64, watermark: Option<DateTime<Utc>>, next_due: DateTime<Utc>) -> Result<(), sqlx::Error>;
    async fn update_target_failure(&self, id: i64, next_due: DateTime<Utc>) -> Result<(), sqlx::Error>;
    async fn record_crawl_run(&self, run: NewCrawlRun) -> Result<(), sqlx::Error>;
    async fn get_recent_crawl_runs(&self, limit: i64) -> Result<Vec<CrawlRun>, sqlx::Error>;
}

#[async_trait]
pub trait PostRepository: Send + Sync {
    async fn find_by_identity_key(&self, identity_key: &str) -> Result<Option<Post>, sqlx::Error>;
    async fn find_by_id(&self, id: i64) -> Result<Option<Post>, sqlx::Error>;
    async fn upsert_post(&self, post: NewPost) -> Result<(Post, bool, bool), sqlx::Error>; // (Post, is_new, is_updated)
    async fn touch_last_seen(&self, id: i64) -> Result<(), sqlx::Error>;
    async fn list_posts(&self, limit: i64, offset: i64) -> Result<Vec<Post>, sqlx::Error>;
}

#[async_trait]
pub trait JobRepository: Send + Sync {
    async fn create_job(&self, job: NewJob) -> Result<(), sqlx::Error>;
    async fn claim_pending_jobs(&self, lease_owner: &str, limit: i64, lease_seconds: i64) -> Result<Vec<Job>, sqlx::Error>;
    async fn complete_job(&self, id: i64) -> Result<(), sqlx::Error>;
    async fn fail_job(&self, id: i64, retry_delay_seconds: i64) -> Result<(), sqlx::Error>;
}

#[async_trait]
pub trait LeadRepository: Send + Sync {
    async fn create_lead(&self, lead: NewLead) -> Result<Lead, sqlx::Error>;
    async fn list_leads(&self, limit: i64, offset: i64) -> Result<Vec<Lead>, sqlx::Error>;
    async fn find_by_post_id(&self, post_id: i64) -> Result<Option<Lead>, sqlx::Error>;
}

#[async_trait]
pub trait OutboxRepository: Send + Sync {
    async fn insert_event(&self, event: NewOutboxEvent) -> Result<(), sqlx::Error>;
    async fn claim_pending_events(&self, lease_owner: &str, limit: i64, lease_seconds: i64) -> Result<Vec<OutboxEvent>, sqlx::Error>;
    async fn mark_delivered(&self, id: i64, remote_id: Option<&str>) -> Result<(), sqlx::Error>;
    async fn mark_retry(&self, id: i64, delay_seconds: i64, error: &str) -> Result<(), sqlx::Error>;
    async fn mark_dead_letter(&self, id: i64, error: &str) -> Result<(), sqlx::Error>;
    async fn list_events(&self, limit: i64, offset: i64) -> Result<Vec<OutboxEvent>, sqlx::Error>;
    async fn retry_dead_letter(&self, id: i64) -> Result<bool, sqlx::Error>;
}

#[async_trait]
pub trait SessionRepository: Send + Sync {
    async fn get_session(&self, id: &str) -> Result<Option<SocialSession>, sqlx::Error>;
    async fn upsert_session(&self, session: &SocialSession) -> Result<(), sqlx::Error>;
    async fn update_status(&self, id: &str, status: &str, incident_key: Option<&str>) -> Result<(), sqlx::Error>;
}
