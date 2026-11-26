//! D1 adapter for device management.
use crate::{db, push::sql, time};
use chrono::{DateTime, Utc};
use nomail_app::{
    auth::AuthContext,
    device::{
        DeviceError, DeviceResult, DeviceSession, DeviceStore, PushPreferences, PushRegistration,
    },
};
use serde_json::{Value, json};

pub(crate) struct Store<'a>(pub &'a db::Store);

fn storage(error: db::StorageError) -> DeviceError {
    match error {
        db::StorageError::Conflict => DeviceError::Conflict,
        db::StorageError::Internal(_) => DeviceError::Internal,
    }
}

impl DeviceStore for Store<'_> {
    async fn push_preferences(&self, device_id: i64) -> DeviceResult<PushPreferences> {
        let rows = self
            .0
            .rows(sql::GET_PREFERENCES, vec![json!(device_id)])
            .await
            .map_err(storage)?;
        let Some(row) = rows.first() else {
            return Ok(PushPreferences::default());
        };
        let settings = row["settings"].as_str().ok_or(DeviceError::Internal)?;
        serde_json::from_str(settings).map_err(|_| DeviceError::Internal)
    }

    async fn set_push_preferences(
        &self,
        actor: &AuthContext,
        preferences: PushPreferences,
        now: DateTime<Utc>,
    ) -> DeviceResult<()> {
        let settings = serde_json::to_string(&preferences).map_err(|_| DeviceError::Internal)?;
        let rows = self
            .0
            .rows(
                sql::SET_PREFERENCES,
                vec![
                    json!(actor.device_id),
                    json!(actor.user_id),
                    json!(actor.session_id),
                    json!(time::format(actor.logged_in_at.timestamp_millis())),
                    json!(now.timestamp()),
                    json!(settings),
                ],
            )
            .await
            .map_err(storage)?;
        rows.first().map(|_| ()).ok_or(DeviceError::Unauthorized)
    }

    async fn devices(&self, uid: i64) -> DeviceResult<Vec<DeviceSession>> {
        let rows = self
            .0
            .rows(
                "SELECT d.*,s.id AS session_id,s.device_id AS session_device_id,
             s.expires_at AS session_expires_at,s.logged_in_at AS session_logged_in_at,
             s.revoked_at AS session_revoked_at,s.deleted_at AS session_deleted_at
             FROM devices d LEFT JOIN sessions s ON s.device_id=d.id WHERE d.user_id=?",
                vec![json!(uid)],
            )
            .await
            .map_err(storage)?;
        rows.into_iter()
            .map(|row| {
                let session = if row["session_id"].is_null() {
                    None
                } else {
                    Some(
                        serde_json::from_value(json!({
                            "id": row["session_id"],
                            "device_id": row["session_device_id"],
                            "expires_at": row["session_expires_at"],
                            "logged_in_at": row["session_logged_in_at"],
                            "revoked_at": row["session_revoked_at"],
                            "deleted_at": row["session_deleted_at"],
                        }))
                        .map_err(|_| DeviceError::Internal)?,
                    )
                };
                Ok(DeviceSession {
                    device: serde_json::from_value(row).map_err(|_| DeviceError::Internal)?,
                    session,
                })
            })
            .collect()
    }

    async fn revoke_device_session(
        &self,
        uid: i64,
        identifier: &str,
        now: DateTime<Utc>,
    ) -> DeviceResult<()> {
        let date = time::format(now.timestamp_millis());
        let result = self.0.0.batch(vec![
            self.0.statement("SELECT id FROM devices WHERE user_id=? AND device_id=? AND deleted_at IS NULL",vec![json!(uid),json!(identifier)]).map_err(storage)?,
            self.0.statement("UPDATE sessions SET revoked_at=?,updated_at=? WHERE device_id IN (SELECT id FROM devices WHERE user_id=? AND device_id=? AND deleted_at IS NULL) AND revoked_at IS NULL AND deleted_at IS NULL",vec![json!(date),json!(date),json!(uid),json!(identifier)]).map_err(storage)?,
        ]).await.map_err(|error| storage(error.into()))?;
        if result[0]
            .results::<Value>()
            .map_err(|error| storage(error.into()))?
            .is_empty()
        {
            Err(DeviceError::NotFound)
        } else {
            Ok(())
        }
    }

    async fn delete_device(&self, uid: i64, identifier: &str) -> DeviceResult<()> {
        // D1 FK cascade deletes the session in the same statement.
        if self.0.rows("DELETE FROM devices WHERE user_id=? AND device_id=? AND deleted_at IS NULL RETURNING id",vec![json!(uid),json!(identifier)]).await.map_err(storage)?.is_empty() {
            Err(DeviceError::NotFound)
        } else {
            Ok(())
        }
    }

    async fn set_target(
        &self,
        actor: &AuthContext,
        target: Option<&PushRegistration>,
        now: DateTime<Utc>,
    ) -> DeviceResult<()> {
        let mut values = vec![
            json!(actor.device_id),
            json!(actor.user_id),
            json!(actor.session_id),
            json!(time::format(actor.logged_in_at.timestamp_millis())),
            json!(now.timestamp()),
        ];
        if let Some(target) = target {
            values.extend([json!(target.token), json!(target.channel.as_str())]);
            if self
                .0
                .rows(sql::SET_TARGET, values)
                .await
                .map_err(|_| DeviceError::Internal)?
                .is_empty()
            {
                return Err(DeviceError::Unauthorized);
            }
        } else {
            self.0
                .run(sql::DROP_TARGET, values)
                .await
                .map_err(|_| DeviceError::Internal)?;
        }
        Ok(())
    }
}
