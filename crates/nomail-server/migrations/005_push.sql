CREATE TABLE push_targets (
    device_id INTEGER PRIMARY KEY REFERENCES devices(id) ON DELETE CASCADE,
    channel TEXT NOT NULL CHECK (channel IN ('apns', 'fcm')),
    token TEXT NOT NULL,
    version TEXT NOT NULL
);

CREATE VIEW active_push_targets AS
SELECT p.*, d.user_id, s.id AS session_id, s.logged_in_at,
       unixepoch(s.expires_at) AS session_expiry
FROM push_targets p
JOIN devices d ON d.id = p.device_id
JOIN users u ON u.id = d.user_id
JOIN sessions s ON s.device_id = d.id
WHERE d.deleted_at IS NULL AND u.deleted_at IS NULL
  AND s.deleted_at IS NULL AND s.revoked_at IS NULL;

-- A fresh login must opt in again. Normal refresh-token rotation does not
-- update logged_in_at or revoke the session.
CREATE TRIGGER clear_push_on_session_update
AFTER UPDATE OF logged_in_at, revoked_at, deleted_at ON sessions
BEGIN
    DELETE FROM push_targets WHERE device_id = OLD.device_id;
END;

CREATE TRIGGER clear_push_on_session_delete
AFTER DELETE ON sessions
BEGIN
    DELETE FROM push_targets WHERE device_id = OLD.device_id;
END;

CREATE TABLE push_jobs (
    delivery_id TEXT PRIMARY KEY NOT NULL,
    -- Native SQLite stores a BLOB UUID; D1 stores a text UUID.
    message_id NOT NULL REFERENCES notify_messages(id) ON DELETE CASCADE,
    device_id INTEGER NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    target_version TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    accepted INTEGER NOT NULL DEFAULT 0,
    next_at INTEGER NOT NULL,
    deadline INTEGER NOT NULL,
    attempt INTEGER NOT NULL DEFAULT 0,
    lease INTEGER NOT NULL DEFAULT 0,
    UNIQUE(message_id, device_id)
);
CREATE INDEX push_jobs_due ON push_jobs(next_at);
CREATE INDEX push_jobs_deadline ON push_jobs(deadline);
CREATE INDEX push_jobs_target ON push_jobs(device_id, target_version);
