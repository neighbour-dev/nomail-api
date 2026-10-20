CREATE TABLE IF NOT EXISTS users (
    id BIGSERIAL PRIMARY KEY,
    created_at TIMESTAMPTZ NOT NULL, updated_at TIMESTAMPTZ NOT NULL, deleted_at TIMESTAMPTZ,
    username VARCHAR(64) NOT NULL, password_hash VARCHAR(255) NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_users_username ON users(username);
CREATE TABLE IF NOT EXISTS devices (
    id BIGSERIAL PRIMARY KEY,
    created_at TIMESTAMPTZ NOT NULL, updated_at TIMESTAMPTZ NOT NULL, deleted_at TIMESTAMPTZ,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_id VARCHAR(128) NOT NULL, name VARCHAR(128),
    platform VARCHAR(32) NOT NULL DEFAULT 'unknown', device_type VARCHAR(32) NOT NULL DEFAULT 'unknown',
    manufacturer VARCHAR(128), model VARCHAR(128), os_version VARCHAR(64), app_version VARCHAR(64),
    app_build VARCHAR(64), locale VARCHAR(32), time_zone VARCHAR(64),
    ip_address VARCHAR(64), user_agent VARCHAR(255), last_seen_at TIMESTAMPTZ
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_user_device ON devices(user_id, device_id);
CREATE TABLE IF NOT EXISTS sessions (
    id BIGSERIAL PRIMARY KEY,
    created_at TIMESTAMPTZ NOT NULL, updated_at TIMESTAMPTZ NOT NULL, deleted_at TIMESTAMPTZ,
    device_id BIGINT NOT NULL UNIQUE REFERENCES devices(id) ON DELETE CASCADE,
    refresh_token_hash VARCHAR(64) NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL, logged_in_at TIMESTAMPTZ NOT NULL, revoked_at TIMESTAMPTZ
);
CREATE TABLE IF NOT EXISTS push_endpoints (
    id BIGSERIAL PRIMARY KEY,
    created_at TIMESTAMPTZ NOT NULL, updated_at TIMESTAMPTZ NOT NULL, deleted_at TIMESTAMPTZ,
    device_id BIGINT NOT NULL UNIQUE REFERENCES devices(id) ON DELETE CASCADE,
    provider VARCHAR(32) NOT NULL, environment VARCHAR(32) NOT NULL DEFAULT 'production',
    encrypted_token TEXT NOT NULL, token_hash VARCHAR(64) NOT NULL UNIQUE,
    enabled BOOLEAN NOT NULL DEFAULT TRUE, registered_at TIMESTAMPTZ NOT NULL,
    invalidated_at TIMESTAMPTZ, last_error VARCHAR(500)
);
CREATE TABLE IF NOT EXISTS webhook_configs (
    id BIGSERIAL PRIMARY KEY,
    created_at TIMESTAMPTZ NOT NULL, updated_at TIMESTAMPTZ NOT NULL, deleted_at TIMESTAMPTZ,
    user_id BIGINT NOT NULL, name VARCHAR(100) NOT NULL, key VARCHAR(64) NOT NULL,
    provider VARCHAR(32) NOT NULL, source VARCHAR(255), encrypted_secret TEXT,
    enabled BOOLEAN NOT NULL DEFAULT TRUE, description VARCHAR(500)
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_webhook_configs_key ON webhook_configs(key);
CREATE TABLE IF NOT EXISTS notify_messages (
    id UUID PRIMARY KEY,
    created_at TIMESTAMPTZ NOT NULL, updated_at TIMESTAMPTZ NOT NULL, deleted_at TIMESTAMPTZ,
    webhook_config_id BIGINT NOT NULL, external_id VARCHAR(255), source VARCHAR(100),
    title VARCHAR(255) NOT NULL, content TEXT, level VARCHAR(30) NOT NULL DEFAULT 'normal',
    status VARCHAR(30) NOT NULL DEFAULT 'unread', url TEXT, raw_payload TEXT, content_type VARCHAR(100),
    pinned BOOLEAN NOT NULL DEFAULT FALSE, expire_at TIMESTAMPTZ, read_at TIMESTAMPTZ
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_notify_message_config_external ON notify_messages(webhook_config_id, external_id);
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
CREATE INDEX IF NOT EXISTS idx_push_endpoints_deleted_at ON push_endpoints(deleted_at);
CREATE INDEX IF NOT EXISTS idx_push_endpoints_enabled ON push_endpoints(enabled);
CREATE INDEX IF NOT EXISTS idx_push_endpoints_provider ON push_endpoints(provider);
CREATE INDEX IF NOT EXISTS idx_webhook_configs_deleted_at ON webhook_configs(deleted_at);
CREATE INDEX IF NOT EXISTS idx_webhook_configs_provider ON webhook_configs(provider);
CREATE INDEX IF NOT EXISTS idx_webhook_configs_source ON webhook_configs(source);
CREATE INDEX IF NOT EXISTS idx_notify_messages_deleted_at ON notify_messages(deleted_at);
CREATE INDEX IF NOT EXISTS idx_notify_messages_level ON notify_messages(level);
CREATE INDEX IF NOT EXISTS idx_notify_messages_pinned ON notify_messages(pinned);
CREATE INDEX IF NOT EXISTS idx_notify_messages_read_at ON notify_messages(read_at);
CREATE INDEX IF NOT EXISTS idx_notify_messages_source ON notify_messages(source);
CREATE INDEX IF NOT EXISTS idx_notify_messages_status ON notify_messages(status);
