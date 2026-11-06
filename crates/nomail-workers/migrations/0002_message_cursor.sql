CREATE INDEX IF NOT EXISTS idx_messages_active_time_id
ON notify_messages(created_at DESC, id DESC)
WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_messages_active_config_time_id
ON notify_messages(webhook_config_id, created_at DESC, id DESC)
WHERE deleted_at IS NULL;
