//! SQLite adapter for device management.
use crate::{
    auth::repository::session,
    db,
    entity::{device, session},
    push::{sql, statement},
};
use chrono::{DateTime, Utc};
use nomail_app::{
    auth::{AuthContext, Session},
    device::{
        Device, DeviceError, DeviceResult, DeviceSession, DeviceStore, PushPreferences,
        PushRegistration,
    },
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter,
    TransactionTrait, sea_query::Expr,
};

pub(crate) struct Store<'a>(pub &'a DatabaseConnection);

fn storage(error: DbErr) -> DeviceError {
    match error.sql_err() {
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_)) => DeviceError::Conflict,
        _ => DeviceError::Internal,
    }
}

pub(crate) fn from_model(m: device::Model) -> Device {
    Device {
        id: m.id,
        user_id: m.user_id,
        device_id: m.device_id,
        name: m.name,
        platform: m.platform,
        device_type: m.device_type,
        manufacturer: m.manufacturer,
        model: m.model,
        os_version: m.os_version,
        app_version: m.app_version,
        app_build: m.app_build,
        locale: m.locale,
        time_zone: m.time_zone,
        ip_address: m.ip_address,
        user_agent: m.user_agent,
        last_seen_at: m.last_seen_at,
        created_at: m.created_at,
        updated_at: m.updated_at,
        deleted_at: m.deleted_at,
    }
}

async fn owned(
    db: &impl ConnectionTrait,
    uid: i64,
    identifier: &str,
) -> DeviceResult<device::Model> {
    device::Entity::find()
        .filter(device::Column::UserId.eq(uid))
        .filter(device::Column::DeviceId.eq(identifier))
        .filter(device::Column::DeletedAt.is_null())
        .one(db)
        .await
        .map_err(storage)?
        .ok_or(DeviceError::NotFound)
}

impl DeviceStore for Store<'_> {
    async fn push_preferences(&self, device_id: i64) -> DeviceResult<PushPreferences> {
        let row = self
            .0
            .query_one(statement(sql::GET_PREFERENCES, [device_id.into()]))
            .await
            .map_err(storage)?;
        let Some(row) = row else {
            return Ok(PushPreferences::default());
        };
        let settings: String = row.try_get("", "settings").map_err(storage)?;
        serde_json::from_str(&settings).map_err(|_| DeviceError::Internal)
    }

    async fn set_push_preferences(
        &self,
        actor: &AuthContext,
        preferences: PushPreferences,
        now: DateTime<Utc>,
    ) -> DeviceResult<()> {
        let settings = serde_json::to_string(&preferences).map_err(|_| DeviceError::Internal)?;
        let row = self
            .0
            .query_one(statement(
                sql::SET_PREFERENCES,
                [
                    actor.device_id.into(),
                    actor.user_id.into(),
                    actor.session_id.into(),
                    actor.logged_in_at.to_rfc3339().into(),
                    now.timestamp().into(),
                    settings.into(),
                ],
            ))
            .await
            .map_err(storage)?;
        row.map(|_| ()).ok_or(DeviceError::Unauthorized)
    }

    async fn devices(&self, uid: i64) -> DeviceResult<Vec<DeviceSession>> {
        let tx = self.0.begin().await.map_err(storage)?;
        let devices = device::Entity::find()
            .filter(device::Column::UserId.eq(uid))
            .all(&tx)
            .await
            .map_err(storage)?;
        let ids: Vec<i64> = devices.iter().map(|d| d.id).collect();
        let sessions = session::Entity::find()
            .filter(session::Column::DeviceId.is_in(ids))
            .all(&tx)
            .await
            .map_err(storage)?;
        let mut sessions: std::collections::HashMap<i64, Session> = sessions
            .into_iter()
            .map(|s| (s.device_id, session(s)))
            .collect();
        tx.commit().await.map_err(storage)?;
        Ok(devices
            .into_iter()
            .map(|d| DeviceSession {
                session: sessions.remove(&d.id),
                device: from_model(d),
            })
            .collect())
    }

    async fn revoke_device_session(
        &self,
        uid: i64,
        identifier: &str,
        now: DateTime<Utc>,
    ) -> DeviceResult<()> {
        let tx = db::write_transaction(self.0).await.map_err(storage)?;
        let d = owned(&tx, uid, identifier).await?;
        session::Entity::update_many()
            .col_expr(session::Column::RevokedAt, Expr::value(now))
            .col_expr(session::Column::UpdatedAt, Expr::value(now))
            .filter(session::Column::DeviceId.eq(d.id))
            .filter(session::Column::RevokedAt.is_null())
            .filter(session::Column::DeletedAt.is_null())
            .exec(&tx)
            .await
            .map_err(storage)?;
        tx.commit().await.map_err(storage)
    }

    async fn delete_device(&self, uid: i64, identifier: &str) -> DeviceResult<()> {
        let tx = db::write_transaction(self.0).await.map_err(storage)?;
        let d = owned(&tx, uid, identifier).await?;
        session::Entity::delete_many()
            .filter(session::Column::DeviceId.eq(d.id))
            .exec(&tx)
            .await
            .map_err(storage)?;
        device::Entity::delete_by_id(d.id)
            .exec(&tx)
            .await
            .map_err(storage)?;
        tx.commit().await.map_err(storage)
    }

    async fn set_target(
        &self,
        actor: &AuthContext,
        target: Option<&PushRegistration>,
        now: DateTime<Utc>,
    ) -> DeviceResult<()> {
        let mut values = vec![
            actor.device_id.into(),
            actor.user_id.into(),
            actor.session_id.into(),
            actor.logged_in_at.to_rfc3339().into(),
            now.timestamp().into(),
        ];
        if let Some(target) = target {
            values.extend([target.token.as_str().into(), target.channel.as_str().into()]);
            let row = self
                .0
                .query_one(statement(sql::SET_TARGET, values))
                .await
                .map_err(|_| DeviceError::Internal)?;
            if row.is_none() {
                return Err(DeviceError::Unauthorized);
            }
        } else {
            self.0
                .execute(statement(sql::DROP_TARGET, values))
                .await
                .map_err(|_| DeviceError::Internal)?;
        }
        Ok(())
    }
}
