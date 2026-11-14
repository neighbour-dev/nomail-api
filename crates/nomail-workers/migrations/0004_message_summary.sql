-- No legacy content conversion or backfill. Existing rows get an empty summary.
DROP INDEX IF EXISTS idx_notify_messages_source;
ALTER TABLE notify_messages ADD COLUMN summary TEXT NOT NULL DEFAULT '';
ALTER TABLE notify_messages DROP COLUMN source;
ALTER TABLE notify_messages DROP COLUMN content;
