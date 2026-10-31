CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT,
    username TEXT NOT NULL UNIQUE, password_hash TEXT NOT NULL
);
CREATE TABLE devices (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_id TEXT NOT NULL, name TEXT,
    platform TEXT NOT NULL DEFAULT 'unknown', device_type TEXT NOT NULL DEFAULT 'unknown',
    manufacturer TEXT, model TEXT, os_version TEXT, app_version TEXT, app_build TEXT,
    locale TEXT, time_zone TEXT, ip_address TEXT, user_agent TEXT, last_seen_at TEXT,
    UNIQUE(user_id, device_id)
);
CREATE TABLE sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT,
    device_id INTEGER NOT NULL UNIQUE REFERENCES devices(id) ON DELETE CASCADE,
    refresh_token_hash TEXT NOT NULL UNIQUE,
    expires_at TEXT NOT NULL, logged_in_at TEXT NOT NULL, revoked_at TEXT
);
CREATE TABLE webhook_configs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT,
    user_id INTEGER NOT NULL, name TEXT NOT NULL, key TEXT NOT NULL UNIQUE,
    provider TEXT NOT NULL, source TEXT, encrypted_secret TEXT,
    enabled INTEGER NOT NULL DEFAULT 1, description TEXT
);
CREATE TABLE notify_messages (
    id TEXT PRIMARY KEY NOT NULL,
    created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT,
    webhook_config_id INTEGER NOT NULL REFERENCES webhook_configs(id),
    provider TEXT NOT NULL, event_type TEXT NOT NULL, event_version TEXT,
    deduplication_scope TEXT, deduplication_key TEXT,
    platform_data TEXT NOT NULL CHECK (json_valid(platform_data) AND json_extract(platform_data, '$.provider') IS provider),
    source TEXT, title TEXT NOT NULL, content TEXT NOT NULL,
    level TEXT NOT NULL CHECK (level IN ('low','normal','high','urgent')),
    status TEXT NOT NULL DEFAULT 'unread' CHECK (status IN ('read','unread')),
    url TEXT, raw_payload TEXT NOT NULL, content_type TEXT NOT NULL,
    pinned INTEGER NOT NULL DEFAULT 0 CHECK (pinned IN (0,1)), expire_at TEXT, read_at TEXT,
    CHECK ((deduplication_scope IS NULL AND deduplication_key IS NULL) OR
        (deduplication_scope IS NOT NULL AND deduplication_key IS NOT NULL AND
         length(deduplication_scope) > 0 AND length(deduplication_key) > 0)),
    CHECK ((status = 'unread' AND read_at IS NULL) OR (status = 'read' AND read_at IS NOT NULL)),
    UNIQUE(webhook_config_id, deduplication_scope, deduplication_key)
);
CREATE INDEX idx_messages_created ON notify_messages(created_at DESC);
CREATE INDEX idx_messages_config ON notify_messages(webhook_config_id);
CREATE INDEX idx_messages_expiry ON notify_messages(expire_at);
CREATE INDEX idx_messages_status ON notify_messages(status);
CREATE INDEX idx_configs_user ON webhook_configs(user_id);
CREATE INDEX idx_devices_user ON devices(user_id);
