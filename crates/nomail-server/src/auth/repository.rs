//! SQLite atomic operations. Authentication policy and HTTP live elsewhere.
use crate::{
    db,
    entity::{device, session, user},
};
use chrono::{DateTime, Utc};
use nomail_core::auth::*;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait, sea_query::Expr,
};

pub struct Store<'a>(pub &'a DatabaseConnection);

#[cfg(test)]
#[path = "repository_tests.rs"]
mod tests;

fn storage(error: DbErr) -> AuthError {
    if matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    ) {
        AuthError::Conflict("resource already exists")
    } else {
        AuthError::Internal(error.to_string())
    }
}
fn account(m: user::Model) -> Account {
    Account {
        id: m.id,
        username: m.username,
        password_hash: m.password_hash,
        deleted_at: m.deleted_at,
    }
}
fn session(m: session::Model) -> Session {
    Session {
        id: m.id,
        device_id: m.device_id,
        expires_at: m.expires_at,
        logged_in_at: m.logged_in_at,
        revoked_at: m.revoked_at,
        deleted_at: m.deleted_at,
    }
}
fn device(m: device::Model) -> Device {
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
async fn owned(db: &impl ConnectionTrait, uid: i64, identifier: &str) -> AuthResult<device::Model> {
    device::Entity::find()
        .filter(device::Column::UserId.eq(uid))
        .filter(device::Column::DeviceId.eq(identifier))
        .filter(device::Column::DeletedAt.is_null())
        .one(db)
        .await
        .map_err(storage)?
        .ok_or(AuthError::NotFound("device not found"))
}

impl AuthStore for Store<'_> {
    async fn has_accounts(&self) -> AuthResult<bool> {
        Ok(user::Entity::find().count(self.0).await.map_err(storage)? > 0)
    }
    async fn initialize_account_if_empty(&self, c: InitialAccountWrite) -> AuthResult<()> {
        let tx = db::write_transaction(self.0).await.map_err(storage)?;
        if user::Entity::find().count(&tx).await.map_err(storage)? == 0 {
            user::ActiveModel {
                username: Set(c.username),
                password_hash: Set(c.password_hash),
                created_at: Set(c.now),
                updated_at: Set(c.now),
                deleted_at: Set(None),
                ..Default::default()
            }
            .insert(&tx)
            .await
            .map_err(storage)?;
        }
        tx.commit().await.map_err(storage)
    }
    async fn account_by_username(&self, name: &str) -> AuthResult<Option<Account>> {
        Ok(user::Entity::find()
            .filter(user::Column::Username.eq(name))
            .one(self.0)
            .await
            .map_err(storage)?
            .map(account))
    }
    async fn account_by_id(&self, id: i64) -> AuthResult<Option<Account>> {
        Ok(user::Entity::find_by_id(id)
            .one(self.0)
            .await
            .map_err(storage)?
            .map(account))
    }
    async fn first_account(&self) -> AuthResult<Option<Account>> {
        Ok(user::Entity::find()
            .filter(user::Column::DeletedAt.is_null())
            .order_by_asc(user::Column::Id)
            .one(self.0)
            .await
            .map_err(storage)?
            .map(account))
    }
    async fn auth_state(&self, id: i64) -> AuthResult<Option<AuthState>> {
        let tx = self.0.begin().await.map_err(storage)?;
        let Some(s) = session::Entity::find_by_id(id)
            .one(&tx)
            .await
            .map_err(storage)?
        else {
            return Ok(None);
        };
        let Some(d) = device::Entity::find_by_id(s.device_id)
            .one(&tx)
            .await
            .map_err(storage)?
        else {
            return Ok(None);
        };
        let Some(a) = user::Entity::find_by_id(d.user_id)
            .one(&tx)
            .await
            .map_err(storage)?
        else {
            return Ok(None);
        };
        tx.commit().await.map_err(storage)?;
        Ok(Some(AuthState {
            account: account(a),
            device: device(d),
            session: session(s),
        }))
    }
    async fn complete_login(&self, c: LoginWrite) -> AuthResult<SessionGrant> {
        if c.max_devices == 0 {
            return Err(AuthError::Internal("invalid device limit".into()));
        }
        let tx = db::write_transaction(self.0).await.map_err(storage)?;
        let a = user::Entity::find_by_id(c.user_id)
            .filter(user::Column::DeletedAt.is_null())
            .one(&tx)
            .await
            .map_err(storage)?
            .ok_or(AuthError::Unauthorized)?;
        if a.password_hash != c.expected_password_hash {
            return Err(AuthError::Unauthorized);
        }
        let existing = device::Entity::find()
            .filter(device::Column::UserId.eq(c.user_id))
            .filter(device::Column::DeviceId.eq(&c.device.device_id))
            .one(&tx)
            .await
            .map_err(storage)?;
        let is_new = existing.is_none();
        let mut d = existing
            .map(Into::into)
            .unwrap_or_else(|| device::ActiveModel {
                user_id: Set(c.user_id),
                device_id: Set(c.device.device_id),
                created_at: Set(c.now),
                ..Default::default()
            });
        d.name = Set(Some(c.device.device_name));
        d.platform = Set(c.device.platform);
        d.device_type = Set(c.device.device_type);
        d.manufacturer = Set(Some(c.device.manufacturer));
        d.model = Set(Some(c.device.model));
        d.os_version = Set(Some(c.device.os_version));
        d.app_version = Set(Some(c.device.app_version));
        d.app_build = Set(Some(c.device.app_build));
        d.locale = Set(Some(c.device.locale));
        d.time_zone = Set(Some(c.device.time_zone));
        d.ip_address = Set(Some(c.meta.ip));
        d.user_agent = Set(Some(c.meta.user_agent));
        d.last_seen_at = Set(Some(c.now));
        d.updated_at = Set(c.now);
        d.deleted_at = Set(None);
        let d = if is_new {
            d.insert(&tx).await
        } else {
            d.update(&tx).await
        }
        .map_err(storage)?;
        let existing = session::Entity::find()
            .filter(session::Column::DeviceId.eq(d.id))
            .one(&tx)
            .await
            .map_err(storage)?;
        let is_new = existing.is_none();
        let mut s = existing
            .map(Into::into)
            .unwrap_or_else(|| session::ActiveModel {
                device_id: Set(d.id),
                created_at: Set(c.now),
                ..Default::default()
            });
        s.refresh_token_hash = Set(c.refresh_token_hash);
        s.expires_at = Set(c.expires_at);
        s.logged_in_at = Set(c.now);
        s.updated_at = Set(c.now);
        s.revoked_at = Set(None);
        s.deleted_at = Set(None);
        let s = if is_new {
            s.insert(&tx).await
        } else {
            s.update(&tx).await
        }
        .map_err(storage)?;
        let ids: Vec<i64> = device::Entity::find()
            .filter(device::Column::UserId.eq(c.user_id))
            .filter(device::Column::DeletedAt.is_null())
            .all(&tx)
            .await
            .map_err(storage)?
            .into_iter()
            .map(|d| d.id)
            .collect();
        let evicted = session::Entity::find()
            .filter(session::Column::DeviceId.is_in(ids))
            .filter(session::Column::DeviceId.ne(d.id))
            .filter(session::Column::DeletedAt.is_null())
            .filter(session::Column::RevokedAt.is_null())
            .filter(session::Column::ExpiresAt.gt(c.now))
            .order_by_desc(session::Column::LoggedInAt)
            .order_by_desc(session::Column::Id)
            .limit(i64::MAX as u64)
            .offset((c.max_devices - 1) as u64)
            .all(&tx)
            .await
            .map_err(storage)?;
        for old in evicted {
            let mut old: session::ActiveModel = old.into();
            old.revoked_at = Set(Some(c.now));
            old.updated_at = Set(c.now);
            old.update(&tx).await.map_err(storage)?;
        }
        tx.commit().await.map_err(storage)?;
        Ok(SessionGrant {
            account: account(a),
            session: session(s),
        })
    }
    async fn rotate_refresh_token(&self, c: RefreshWrite) -> AuthResult<SessionGrant> {
        let tx = db::write_transaction(self.0).await.map_err(storage)?;
        let s = session::Entity::find()
            .filter(session::Column::RefreshTokenHash.eq(&c.previous_hash))
            .filter(session::Column::DeletedAt.is_null())
            .filter(session::Column::RevokedAt.is_null())
            .filter(session::Column::ExpiresAt.gt(c.now))
            .one(&tx)
            .await
            .map_err(storage)?
            .ok_or(AuthError::Unauthorized)?;
        let d = device::Entity::find_by_id(s.device_id)
            .filter(device::Column::DeletedAt.is_null())
            .one(&tx)
            .await
            .map_err(storage)?
            .ok_or(AuthError::Unauthorized)?;
        let a = user::Entity::find_by_id(d.user_id)
            .filter(user::Column::DeletedAt.is_null())
            .one(&tx)
            .await
            .map_err(storage)?
            .ok_or(AuthError::Unauthorized)?;
        let mut s: session::ActiveModel = s.into();
        s.refresh_token_hash = Set(c.new_hash);
        s.expires_at = Set(c.expires_at);
        s.updated_at = Set(c.now);
        let s = s.update(&tx).await.map_err(storage)?;
        let mut d: device::ActiveModel = d.into();
        d.ip_address = Set(Some(c.meta.ip));
        d.user_agent = Set(Some(c.meta.user_agent));
        d.last_seen_at = Set(Some(c.now));
        d.updated_at = Set(c.now);
        d.update(&tx).await.map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok(SessionGrant {
            account: account(a),
            session: session(s),
        })
    }
    async fn revoke_refresh_token(&self, hash: &str, now: DateTime<Utc>) -> AuthResult<bool> {
        Ok(session::Entity::update_many()
            .col_expr(session::Column::RevokedAt, Expr::value(now))
            .col_expr(session::Column::UpdatedAt, Expr::value(now))
            .filter(session::Column::RefreshTokenHash.eq(hash))
            .filter(session::Column::RevokedAt.is_null())
            .filter(session::Column::DeletedAt.is_null())
            .filter(session::Column::ExpiresAt.gt(now))
            .exec(self.0)
            .await
            .map_err(storage)?
            .rows_affected
            > 0)
    }
    async fn update_account(&self, c: AccountWrite) -> AuthResult<Account> {
        let tx = db::write_transaction(self.0).await.map_err(storage)?;
        let a = user::Entity::find_by_id(c.user_id)
            .filter(user::Column::DeletedAt.is_null())
            .one(&tx)
            .await
            .map_err(storage)?
            .ok_or(AuthError::Unauthorized)?;
        if a.password_hash != c.expected_password_hash {
            return Err(AuthError::Unauthorized);
        }
        let mut a: user::ActiveModel = a.into();
        a.username = Set(c.username);
        a.updated_at = Set(c.now);
        if let Some(hash) = c.new_password_hash {
            a.password_hash = Set(hash);
            let ids: Vec<i64> = device::Entity::find()
                .filter(device::Column::UserId.eq(c.user_id))
                .all(&tx)
                .await
                .map_err(storage)?
                .into_iter()
                .map(|d| d.id)
                .collect();
            session::Entity::update_many()
                .col_expr(session::Column::RevokedAt, Expr::value(c.now))
                .col_expr(session::Column::UpdatedAt, Expr::value(c.now))
                .filter(session::Column::DeviceId.is_in(ids))
                .exec(&tx)
                .await
                .map_err(storage)?;
        }
        let a = a.update(&tx).await.map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok(account(a))
    }
    async fn touch_device(
        &self,
        id: i64,
        before: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> AuthResult<()> {
        device::Entity::update_many()
            .col_expr(device::Column::LastSeenAt, Expr::value(now))
            .col_expr(device::Column::UpdatedAt, Expr::value(now))
            .filter(device::Column::Id.eq(id))
            .filter(
                device::Column::LastSeenAt
                    .is_null()
                    .or(device::Column::LastSeenAt.lt(before)),
            )
            .exec(self.0)
            .await
            .map_err(storage)?;
        Ok(())
    }
    async fn devices(&self, uid: i64) -> AuthResult<Vec<DeviceSession>> {
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
                device: device(d),
            })
            .collect())
    }
    async fn revoke_device_session(
        &self,
        uid: i64,
        identifier: &str,
        now: DateTime<Utc>,
    ) -> AuthResult<()> {
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
    async fn delete_device(&self, uid: i64, identifier: &str) -> AuthResult<()> {
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
}
