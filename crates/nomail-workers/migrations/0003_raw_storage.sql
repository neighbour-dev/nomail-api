-- Incompatible storage change: clear messages before upgrading. No historical
-- JSON conversion, request replay, or credential-dependent migration is provided.
CREATE TABLE nomail_raw_storage_upgrade_guard (
    has_messages INTEGER NOT NULL
        CONSTRAINT clear_notify_messages_before_raw_storage CHECK (has_messages = 0)
);
INSERT INTO nomail_raw_storage_upgrade_guard
    SELECT EXISTS (SELECT 1 FROM notify_messages LIMIT 1);
ALTER TABLE notify_messages ADD COLUMN raw_headers TEXT NOT NULL DEFAULT '[]'
    CHECK (json_valid(raw_headers) AND json_type(raw_headers) = 'array');
ALTER TABLE notify_messages DROP COLUMN platform_data;
DROP TABLE nomail_raw_storage_upgrade_guard;
