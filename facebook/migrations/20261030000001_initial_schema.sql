-- Migration: 001_initial_schema.sql
-- Compatible with SQLite and MySQL

CREATE TABLE IF NOT EXISTS social_sessions (
    id VARCHAR(64) PRIMARY KEY,
    account_alias VARCHAR(255) NOT NULL,
    platform VARCHAR(32) NOT NULL DEFAULT 'facebook',
    status VARCHAR(32) NOT NULL DEFAULT 'READY',
    session_version INT NOT NULL DEFAULT 1,
    lease_owner VARCHAR(64) NULL,
    lease_until TIMESTAMP NULL,
    last_verified_at TIMESTAMP NULL,
    incident_key VARCHAR(128) NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS social_watch_targets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    platform VARCHAR(32) NOT NULL DEFAULT 'facebook',
    group_id VARCHAR(128) NOT NULL,
    display_name VARCHAR(255) NOT NULL,
    session_id VARCHAR(64) NOT NULL DEFAULT 'primary',
    enabled BOOLEAN NOT NULL DEFAULT 1,
    interval_seconds INT NOT NULL DEFAULT 300,
    next_due_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    ordering VARCHAR(32) NOT NULL DEFAULT 'chronological',
    watermark TIMESTAMP NULL,
    last_success_at TIMESTAMP NULL,
    failure_count INT NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS social_crawl_runs (
    run_id VARCHAR(64) PRIMARY KEY,
    target_id INT NOT NULL,
    started_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    finished_at TIMESTAMP NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'RUNNING',
    observed_count INT NOT NULL DEFAULT 0,
    new_count INT NOT NULL DEFAULT 0,
    stop_reason VARCHAR(64) NULL,
    coverage VARCHAR(32) NOT NULL DEFAULT 'incremental',
    error_code VARCHAR(64) NULL,
    parser_version VARCHAR(64) NOT NULL DEFAULT 'social-listener-rust-v3.0.0'
);

CREATE TABLE IF NOT EXISTS social_posts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    platform VARCHAR(32) NOT NULL DEFAULT 'facebook',
    group_id VARCHAR(128) NOT NULL,
    post_id VARCHAR(128) NOT NULL,
    canonical_url TEXT NOT NULL,
    identity_key VARCHAR(255) NOT NULL UNIQUE,
    content_hash VARCHAR(64) NOT NULL,
    text TEXT NOT NULL,
    text_complete BOOLEAN NOT NULL DEFAULT 1,
    author_display VARCHAR(255) NOT NULL DEFAULT '',
    source_created_at TIMESTAMP NULL,
    first_seen_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_seen_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    revision INT NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS social_post_aliases (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    platform VARCHAR(32) NOT NULL DEFAULT 'facebook',
    canonical_url_hash VARCHAR(64) NOT NULL UNIQUE,
    social_post_id INT NOT NULL
);

CREATE TABLE IF NOT EXISTS social_jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind VARCHAR(32) NOT NULL DEFAULT 'CLASSIFY',
    post_id INT NOT NULL,
    revision INT NOT NULL DEFAULT 1,
    status VARCHAR(32) NOT NULL DEFAULT 'PENDING',
    available_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    attempts INT NOT NULL DEFAULT 0,
    lease_owner VARCHAR(64) NULL,
    lease_until TIMESTAMP NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(kind, post_id, revision)
);

CREATE TABLE IF NOT EXISTS social_leads (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    post_id INT NOT NULL,
    intent VARCHAR(64) NOT NULL,
    confidence FLOAT NOT NULL,
    reason TEXT NOT NULL,
    rule_version VARCHAR(32) NOT NULL DEFAULT 'v3.0.0',
    model_version VARCHAR(64) NULL,
    review_status VARCHAR(32) NOT NULL DEFAULT 'APPROVED',
    location VARCHAR(255) NULL,
    requested_date VARCHAR(255) NULL,
    summary TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS social_outbox (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_key VARCHAR(255) NOT NULL UNIQUE,
    payload TEXT NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'PENDING',
    available_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    attempts INT NOT NULL DEFAULT 0,
    lease_owner VARCHAR(64) NULL,
    lease_until TIMESTAMP NULL,
    last_error TEXT NULL,
    delivered_at TIMESTAMP NULL,
    remote_message_id VARCHAR(128) NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
