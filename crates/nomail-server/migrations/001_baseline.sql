CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT,
    username VARCHAR(64) NOT NULL, password_hash VARCHAR(255) NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_users_username ON users(username);
CREATE TABLE IF NOT EXISTS devices (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_id VARCHAR(128) NOT NULL, name VARCHAR(128),
    platform VARCHAR(32) NOT NULL DEFAULT 'unknown', device_type VARCHAR(32) NOT NULL DEFAULT 'unknown',
    manufacturer VARCHAR(128), model VARCHAR(128), os_version VARCHAR(64), app_version VARCHAR(64),
    app_build VARCHAR(64), locale VARCHAR(32), time_zone VARCHAR(64),
    ip_address VARCHAR(64), user_agent VARCHAR(255), last_seen_at TEXT
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_user_device ON devices(user_id, device_id);
CREATE TABLE IF NOT EXISTS sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT,
    device_id BIGINT NOT NULL UNIQUE REFERENCES devices(id) ON DELETE CASCADE,
    refresh_token_hash VARCHAR(64) NOT NULL UNIQUE,
    expires_at TEXT NOT NULL, logged_in_at TEXT NOT NULL, revoked_at TEXT
);
CREATE TABLE IF NOT EXISTS webhook_configs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT,
    user_id BIGINT NOT NULL, name VARCHAR(100) NOT NULL, key VARCHAR(64) NOT NULL,
    provider VARCHAR(32) NOT NULL, source VARCHAR(255), encrypted_secret TEXT,
    enabled BOOLEAN NOT NULL DEFAULT TRUE, description VARCHAR(500)
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_webhook_configs_key ON webhook_configs(key);
CREATE TABLE IF NOT EXISTS notify_messages (
    id BLOB PRIMARY KEY NOT NULL,
    created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT,
    webhook_config_id INTEGER NOT NULL REFERENCES webhook_configs(id),
    provider TEXT NOT NULL, event_type TEXT NOT NULL, event_version TEXT,
    deduplication_scope TEXT, deduplication_key TEXT,
    platform_data TEXT NOT NULL CHECK (json_valid(platform_data) AND json_extract(platform_data, '$.provider') IS provider),
    source VARCHAR(100), title VARCHAR(255) NOT NULL, content TEXT NOT NULL,
    level TEXT NOT NULL CHECK (level IN ('low','normal','high','urgent')),
    status TEXT NOT NULL DEFAULT 'unread' CHECK (status IN ('read','unread')),
    url TEXT, raw_payload TEXT NOT NULL, content_type TEXT NOT NULL,
    pinned INTEGER NOT NULL DEFAULT 0 CHECK (pinned IN (0,1)), expire_at TEXT, read_at TEXT,
    CHECK ((deduplication_scope IS NULL AND deduplication_key IS NULL) OR
        (deduplication_scope IS NOT NULL AND deduplication_key IS NOT NULL AND
         length(deduplication_scope) > 0 AND length(deduplication_key) > 0)),
    CHECK ((status = 'unread' AND read_at IS NULL) OR (status = 'read' AND read_at IS NOT NULL))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_notify_message_config_dedup ON notify_messages(webhook_config_id, deduplication_scope, deduplication_key);
CREATE INDEX IF NOT EXISTS idx_notify_messages_created_at ON notify_messages(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_notify_messages_webhook_config_id ON notify_messages(webhook_config_id);
CREATE INDEX IF NOT EXISTS idx_notify_messages_expire_at ON notify_messages(expire_at);
CREATE INDEX IF NOT EXISTS idx_webhook_configs_user_id ON webhook_configs(user_id);
CREATE INDEX IF NOT EXISTS idx_devices_user_id ON devices(user_id);
CREATE INDEX IF NOT EXISTS idx_users_deleted_at ON users(deleted_at);
CREATE INDEX IF NOT EXISTS idx_devices_deleted_at ON devices(deleted_at);
CREATE INDEX IF NOT EXISTS idx_devices_device_type ON devices(device_type);
CREATE INDEX IF NOT EXISTS idx_devices_last_seen_at ON devices(last_seen_at);
CREATE INDEX IF NOT EXISTS idx_devices_platform ON devices(platform);
CREATE INDEX IF NOT EXISTS idx_sessions_deleted_at ON sessions(deleted_at);
CREATE INDEX IF NOT EXISTS idx_sessions_expires_at ON sessions(expires_at);
CREATE INDEX IF NOT EXISTS idx_sessions_logged_in_at ON sessions(logged_in_at);
CREATE INDEX IF NOT EXISTS idx_webhook_configs_deleted_at ON webhook_configs(deleted_at);
CREATE INDEX IF NOT EXISTS idx_webhook_configs_provider ON webhook_configs(provider);
CREATE INDEX IF NOT EXISTS idx_webhook_configs_source ON webhook_configs(source);
CREATE INDEX IF NOT EXISTS idx_notify_messages_deleted_at ON notify_messages(deleted_at);
CREATE INDEX IF NOT EXISTS idx_notify_messages_level ON notify_messages(level);
CREATE INDEX IF NOT EXISTS idx_notify_messages_pinned ON notify_messages(pinned);
CREATE INDEX IF NOT EXISTS idx_notify_messages_read_at ON notify_messages(read_at);
CREATE INDEX IF NOT EXISTS idx_notify_messages_source ON notify_messages(source);
CREATE INDEX IF NOT EXISTS idx_notify_messages_status ON notify_messages(status);
CREATE TABLE IF NOT EXISTS nomail_write_lock (id INTEGER PRIMARY KEY CHECK (id = 1));
INSERT OR IGNORE INTO nomail_write_lock (id) VALUES (1);
