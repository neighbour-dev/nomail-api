//! Storage statements shared by the SQLite and D1 adapters, not App policy.

pub const SET_TARGET: &str = "
INSERT INTO push_targets(device_id, token, channel, version)
SELECT d.id, ?6, ?7, lower(hex(randomblob(16)))
FROM devices d JOIN users u ON u.id=d.user_id JOIN sessions s ON s.device_id=d.id
WHERE d.id=?1 AND d.user_id=?2 AND s.id=?3
  AND s.logged_in_at=?4 AND unixepoch(s.expires_at)>?5
  AND d.deleted_at IS NULL AND u.deleted_at IS NULL
  AND s.deleted_at IS NULL AND s.revoked_at IS NULL
ON CONFLICT(device_id) DO UPDATE SET
    version=CASE WHEN token=excluded.token AND channel=excluded.channel
                 THEN version ELSE excluded.version END,
    token=excluded.token,
    channel=excluded.channel
RETURNING device_id";

pub const DROP_TARGET: &str = "
DELETE FROM push_targets WHERE device_id=?1 AND EXISTS (
    SELECT 1 FROM active_push_targets p WHERE p.device_id=?1 AND p.user_id=?2
      AND p.session_id=?3 AND p.logged_in_at=?4 AND p.session_expiry>?5
)";

pub const GET_PREFERENCES: &str = "
SELECT json_set(levels, '$.enabled', json(CASE enabled WHEN 1 THEN 'true' ELSE 'false' END)) AS settings
FROM push_preferences WHERE device_id=?1";
pub const SET_PREFERENCES: &str = "
INSERT INTO push_preferences(device_id, enabled, levels)
SELECT d.id, json_extract(?6, '$.enabled'), json_remove(?6, '$.enabled')
FROM devices d JOIN users u ON u.id=d.user_id JOIN sessions s ON s.device_id=d.id
WHERE d.id=?1 AND d.user_id=?2 AND s.id=?3
  AND s.logged_in_at=?4 AND unixepoch(s.expires_at)>?5
  AND d.deleted_at IS NULL AND u.deleted_at IS NULL
  AND s.deleted_at IS NULL AND s.revoked_at IS NULL
ON CONFLICT(device_id) DO UPDATE SET enabled=excluded.enabled, levels=excluded.levels
RETURNING device_id";

// In D1 this runs directly after the message INSERT in the same atomic batch.
// `changes()` binds the enqueue to the winning insert (not a duplicate).
pub const ENQUEUE: &str = "
INSERT INTO push_jobs(delivery_id,message_id,device_id,target_version,title,body,next_at,deadline)
SELECT lower(hex(randomblob(6)) || '4' || substr(hex(randomblob(2)), 2)
       || substr('89ab', (random() & 3) + 1, 1) || substr(hex(randomblob(8)), 2)),
       m.id, p.device_id, p.version, ?2, ?3, ?4, ?5
FROM notify_messages m JOIN webhook_configs w ON w.id=m.webhook_config_id
JOIN active_push_targets p ON p.user_id=w.user_id
JOIN push_preferences pref ON pref.device_id=p.device_id
WHERE changes()=1 AND m.id=?1 AND p.session_expiry>?4
  AND pref.enabled=1
  AND json_extract(pref.levels, '$.' || m.level)=1";

pub const EXPIRE: &str = "DELETE FROM push_jobs WHERE deadline<=?1";

// Recheck eligibility on every claim. The credential version is immutable for
// the job, so changing the channel or token never mutates a command that might have arrived.
pub const CLAIM: &str = "
UPDATE push_jobs SET next_at=?2, lease=lease+1, attempt=attempt+1
WHERE delivery_id=(
    SELECT j.delivery_id FROM push_jobs j
    JOIN active_push_targets p ON p.device_id=j.device_id AND p.version=j.target_version
    JOIN notify_messages m ON m.id=j.message_id
    LEFT JOIN push_preferences pref ON pref.device_id=p.device_id
    WHERE j.next_at<=?1 AND j.deadline>?1 AND p.session_expiry>?1
      AND (j.accepted=1 OR
          (m.deleted_at IS NULL AND m.status='unread'
           AND pref.enabled=1
           AND json_extract(pref.levels, '$.' || m.level)=1
           AND (m.expire_at IS NULL OR unixepoch(m.expire_at)>?1)))
    ORDER BY j.next_at, j.delivery_id LIMIT 1
)
RETURNING delivery_id,
    CASE typeof(message_id) WHEN 'blob' THEN lower(hex(message_id)) ELSE message_id END AS message_id,
    (SELECT token FROM push_targets WHERE device_id=push_jobs.device_id) AS token,
    (SELECT channel FROM push_targets WHERE device_id=push_jobs.device_id) AS channel,
    title, body, accepted, attempt, lease";

pub const RESCHEDULE: &str = "
UPDATE push_jobs SET accepted=?3, next_at=?4, deadline=COALESCE(?5, deadline),
    attempt=CASE WHEN accepted=0 AND ?3=1 THEN 0 ELSE attempt END
WHERE delivery_id=?1 AND lease=?2";

pub const REMOVE: &str = "DELETE FROM push_jobs WHERE delivery_id=?1 AND lease=?2";

pub const INVALIDATE: &str = "
DELETE FROM push_targets WHERE EXISTS (
    SELECT 1 FROM push_jobs j WHERE j.delivery_id=?1 AND j.lease=?2
      AND j.device_id=push_targets.device_id AND j.target_version=push_targets.version
)";
