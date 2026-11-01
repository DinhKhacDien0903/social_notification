use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Pool, Sqlite};

use crate::domain::{
    CrawlRun, Job, JobRepository, Lead, LeadRepository, NewCrawlRun, NewJob, NewLead, NewOutboxEvent,
    NewPost, NewWatchTarget, OutboxEvent, OutboxRepository, Post, PostRepository, SessionRepository,
    SocialSession, TargetRepository, WatchTarget,
};

#[derive(Clone)]
pub struct SqliteDatabase {
    pool: Pool<Sqlite>,
}

impl SqliteDatabase {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &Pool<Sqlite> {
        &self.pool
    }
}

#[async_trait]
impl TargetRepository for SqliteDatabase {
    async fn get_all_targets(&self) -> Result<Vec<WatchTarget>, sqlx::Error> {
        sqlx::query_as::<_, WatchTarget>("SELECT * FROM social_watch_targets ORDER BY id ASC")
            .fetch_all(&self.pool)
            .await
    }

    async fn get_target_by_id(&self, id: i64) -> Result<Option<WatchTarget>, sqlx::Error> {
        sqlx::query_as::<_, WatchTarget>("SELECT * FROM social_watch_targets WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    async fn create_target(&self, target: NewWatchTarget) -> Result<WatchTarget, sqlx::Error> {
        let res = sqlx::query(
            r#"
            INSERT INTO social_watch_targets (platform, group_id, display_name, session_id, enabled, interval_seconds, ordering)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&target.platform)
        .bind(&target.group_id)
        .bind(&target.display_name)
        .bind(&target.session_id)
        .bind(target.enabled)
        .bind(target.interval_seconds)
        .bind(&target.ordering)
        .execute(&self.pool)
        .await?;

        let id = res.last_insert_rowid();
        self.get_target_by_id(id).await?.ok_or(sqlx::Error::RowNotFound)
    }

    async fn toggle_target(&self, id: i64, enabled: bool) -> Result<bool, sqlx::Error> {
        let res = sqlx::query("UPDATE social_watch_targets SET enabled = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(enabled)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    async fn claim_due_targets(&self, _lease_owner: &str, limit: i64) -> Result<Vec<WatchTarget>, sqlx::Error> {
        sqlx::query_as::<_, WatchTarget>(
            r#"
            SELECT * FROM social_watch_targets 
            WHERE enabled = 1 AND next_due_at <= CURRENT_TIMESTAMP 
            ORDER BY next_due_at ASC 
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    async fn update_target_success(&self, id: i64, watermark: Option<DateTime<Utc>>, next_due: DateTime<Utc>) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE social_watch_targets 
            SET watermark = COALESCE(?, watermark), 
                last_success_at = CURRENT_TIMESTAMP, 
                failure_count = 0, 
                next_due_at = ?,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = ?
            "#,
        )
        .bind(watermark)
        .bind(next_due)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn update_target_failure(&self, id: i64, next_due: DateTime<Utc>) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE social_watch_targets 
            SET failure_count = failure_count + 1, 
                next_due_at = ?,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = ?
            "#,
        )
        .bind(next_due)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn record_crawl_run(&self, run: NewCrawlRun) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO social_crawl_runs (run_id, target_id, finished_at, status, observed_count, new_count, stop_reason, coverage, error_code, parser_version)
            VALUES (?, ?, CURRENT_TIMESTAMP, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&run.run_id)
        .bind(run.target_id)
        .bind(&run.status)
        .bind(run.observed_count)
        .bind(run.new_count)
        .bind(&run.stop_reason)
        .bind(&run.coverage)
        .bind(&run.error_code)
        .bind(&run.parser_version)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_recent_crawl_runs(&self, limit: i64) -> Result<Vec<CrawlRun>, sqlx::Error> {
        sqlx::query_as::<_, CrawlRun>("SELECT * FROM social_crawl_runs ORDER BY started_at DESC LIMIT ?")
            .bind(limit)
            .fetch_all(&self.pool)
            .await
    }
}

#[async_trait]
impl PostRepository for SqliteDatabase {
    async fn find_by_identity_key(&self, identity_key: &str) -> Result<Option<Post>, sqlx::Error> {
        sqlx::query_as::<_, Post>("SELECT * FROM social_posts WHERE identity_key = ?")
            .bind(identity_key)
            .fetch_optional(&self.pool)
            .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<Post>, sqlx::Error> {
        sqlx::query_as::<_, Post>("SELECT * FROM social_posts WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    async fn upsert_post(&self, new_post: NewPost) -> Result<(Post, bool, bool), sqlx::Error> {
        if let Some(existing) = self.find_by_identity_key(&new_post.identity_key).await? {
            if existing.content_hash != new_post.content_hash {
                // Bài cũ bị người dùng sửa nội dung -> tăng revision
                let new_revision = existing.revision + 1;
                sqlx::query(
                    r#"
                    UPDATE social_posts 
                    SET content_hash = ?, text = ?, revision = ?, last_seen_at = CURRENT_TIMESTAMP
                    WHERE id = ?
                    "#,
                )
                .bind(&new_post.content_hash)
                .bind(&new_post.text)
                .bind(new_revision)
                .bind(existing.id)
                .execute(&self.pool)
                .await?;

                let updated = self.find_by_id(existing.id).await?.unwrap();
                Ok((updated, false, true))
            } else {
                // Bài cũ không đổi nội dung
                sqlx::query("UPDATE social_posts SET last_seen_at = CURRENT_TIMESTAMP WHERE id = ?")
                    .bind(existing.id)
                    .execute(&self.pool)
                    .await?;
                Ok((existing, false, false))
            }
        } else {
            // Bài viết mới hoàn toàn
            let res = sqlx::query(
                r#"
                INSERT INTO social_posts (platform, group_id, post_id, canonical_url, identity_key, content_hash, text, text_complete, author_display, source_created_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&new_post.platform)
            .bind(&new_post.group_id)
            .bind(&new_post.post_id)
            .bind(&new_post.canonical_url)
            .bind(&new_post.identity_key)
            .bind(&new_post.content_hash)
            .bind(&new_post.text)
            .bind(new_post.text_complete)
            .bind(&new_post.author_display)
            .bind(new_post.source_created_at)
            .execute(&self.pool)
            .await?;

            let id = res.last_insert_rowid();
            let post = self.find_by_id(id).await?.ok_or(sqlx::Error::RowNotFound)?;
            Ok((post, true, false))
        }
    }

    async fn touch_last_seen(&self, id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE social_posts SET last_seen_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn list_posts(&self, limit: i64, offset: i64) -> Result<Vec<Post>, sqlx::Error> {
        sqlx::query_as::<_, Post>("SELECT * FROM social_posts ORDER BY id DESC LIMIT ? OFFSET ?")
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
    }
}

#[async_trait]
impl JobRepository for SqliteDatabase {
    async fn create_job(&self, job: NewJob) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO social_jobs (kind, post_id, revision, status)
            VALUES (?, ?, ?, 'PENDING')
            ON CONFLICT(kind, post_id, revision) DO NOTHING
            "#,
        )
        .bind(&job.kind)
        .bind(job.post_id)
        .bind(job.revision)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn claim_pending_jobs(&self, lease_owner: &str, limit: i64, lease_seconds: i64) -> Result<Vec<Job>, sqlx::Error> {
        // Cập nhật lease cho các job PENDING
        sqlx::query(
            r#"
            UPDATE social_jobs 
            SET status = 'PROCESSING', lease_owner = ?, lease_until = datetime('now', '+' || ? || ' seconds')
            WHERE id IN (
                SELECT id FROM social_jobs 
                WHERE (status = 'PENDING' OR (status = 'PROCESSING' AND lease_until < CURRENT_TIMESTAMP))
                  AND available_at <= CURRENT_TIMESTAMP
                ORDER BY id ASC
                LIMIT ?
            )
            "#,
        )
        .bind(lease_owner)
        .bind(lease_seconds)
        .bind(limit)
        .execute(&self.pool)
        .await?;

        // Lấy danh sách job vừa claim
        sqlx::query_as::<_, Job>(
            r#"
            SELECT * FROM social_jobs 
            WHERE lease_owner = ? AND status = 'PROCESSING'
            LIMIT ?
            "#,
        )
        .bind(lease_owner)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    async fn complete_job(&self, id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE social_jobs SET status = 'COMPLETED', updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn fail_job(&self, id: i64, retry_delay_seconds: i64) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE social_jobs 
            SET status = 'PENDING', attempts = attempts + 1, available_at = datetime('now', '+' || ? || ' seconds'), updated_at = CURRENT_TIMESTAMP
            WHERE id = ?
            "#,
        )
        .bind(retry_delay_seconds)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[async_trait]
impl LeadRepository for SqliteDatabase {
    async fn create_lead(&self, lead: NewLead) -> Result<Lead, sqlx::Error> {
        let res = sqlx::query(
            r#"
            INSERT INTO social_leads (post_id, intent, confidence, reason, rule_version, model_version, review_status, location, requested_date, summary)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(lead.post_id)
        .bind(&lead.intent)
        .bind(lead.confidence)
        .bind(&lead.reason)
        .bind(&lead.rule_version)
        .bind(&lead.model_version)
        .bind(&lead.review_status)
        .bind(&lead.location)
        .bind(&lead.requested_date)
        .bind(&lead.summary)
        .execute(&self.pool)
        .await?;

        let id = res.last_insert_rowid();
        sqlx::query_as::<_, Lead>("SELECT * FROM social_leads WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
    }

    async fn list_leads(&self, limit: i64, offset: i64) -> Result<Vec<Lead>, sqlx::Error> {
        sqlx::query_as::<_, Lead>("SELECT * FROM social_leads ORDER BY id DESC LIMIT ? OFFSET ?")
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
    }

    async fn find_by_post_id(&self, post_id: i64) -> Result<Option<Lead>, sqlx::Error> {
        sqlx::query_as::<_, Lead>("SELECT * FROM social_leads WHERE post_id = ?")
            .bind(post_id)
            .fetch_optional(&self.pool)
            .await
    }
}

#[async_trait]
impl OutboxRepository for SqliteDatabase {
    async fn insert_event(&self, event: NewOutboxEvent) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO social_outbox (event_key, payload, status)
            VALUES (?, ?, 'PENDING')
            ON CONFLICT(event_key) DO NOTHING
            "#,
        )
        .bind(&event.event_key)
        .bind(&event.payload)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn claim_pending_events(&self, lease_owner: &str, limit: i64, lease_seconds: i64) -> Result<Vec<OutboxEvent>, sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE social_outbox 
            SET status = 'PROCESSING', lease_owner = ?, lease_until = datetime('now', '+' || ? || ' seconds')
            WHERE id IN (
                SELECT id FROM social_outbox 
                WHERE (status = 'PENDING' OR (status = 'PROCESSING' AND lease_until < CURRENT_TIMESTAMP))
                  AND available_at <= CURRENT_TIMESTAMP
                ORDER BY id ASC
                LIMIT ?
            )
            "#,
        )
        .bind(lease_owner)
        .bind(lease_seconds)
        .bind(limit)
        .execute(&self.pool)
        .await?;

        sqlx::query_as::<_, OutboxEvent>(
            r#"
            SELECT * FROM social_outbox 
            WHERE lease_owner = ? AND status = 'PROCESSING'
            LIMIT ?
            "#,
        )
        .bind(lease_owner)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    async fn mark_delivered(&self, id: i64, remote_id: Option<&str>) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE social_outbox 
            SET status = 'DELIVERED', delivered_at = CURRENT_TIMESTAMP, remote_message_id = ?, updated_at = CURRENT_TIMESTAMP
            WHERE id = ?
            "#,
        )
        .bind(remote_id)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn mark_retry(&self, id: i64, delay_seconds: i64, error: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE social_outbox 
            SET status = 'PENDING', attempts = attempts + 1, available_at = datetime('now', '+' || ? || ' seconds'), last_error = ?, updated_at = CURRENT_TIMESTAMP
            WHERE id = ?
            "#,
        )
        .bind(delay_seconds)
        .bind(error)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn mark_dead_letter(&self, id: i64, error: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE social_outbox 
            SET status = 'DEAD_LETTER', last_error = ?, updated_at = CURRENT_TIMESTAMP
            WHERE id = ?
            "#,
        )
        .bind(error)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn list_events(&self, limit: i64, offset: i64) -> Result<Vec<OutboxEvent>, sqlx::Error> {
        sqlx::query_as::<_, OutboxEvent>("SELECT * FROM social_outbox ORDER BY id DESC LIMIT ? OFFSET ?")
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
    }

    async fn retry_dead_letter(&self, id: i64) -> Result<bool, sqlx::Error> {
        let res = sqlx::query("UPDATE social_outbox SET status = 'PENDING', attempts = 0, available_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }
}

#[async_trait]
impl SessionRepository for SqliteDatabase {
    async fn get_session(&self, id: &str) -> Result<Option<SocialSession>, sqlx::Error> {
        sqlx::query_as::<_, SocialSession>("SELECT * FROM social_sessions WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    async fn upsert_session(&self, session: &SocialSession) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO social_sessions (id, account_alias, platform, status, session_version, last_verified_at, incident_key)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                account_alias = excluded.account_alias,
                status = excluded.status,
                session_version = excluded.session_version,
                last_verified_at = excluded.last_verified_at,
                incident_key = excluded.incident_key,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(&session.id)
        .bind(&session.account_alias)
        .bind(&session.platform)
        .bind(&session.status)
        .bind(session.session_version)
        .bind(session.last_verified_at)
        .bind(&session.incident_key)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn update_status(&self, id: &str, status: &str, incident_key: Option<&str>) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE social_sessions SET status = ?, incident_key = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(status)
            .bind(incident_key)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
