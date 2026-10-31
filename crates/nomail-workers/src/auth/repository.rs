//! D1 implements the same atomic auth contract as the native SQLite adapter.
use crate::{db, time};
use chrono::{DateTime, Utc};
use nomail_app::auth::*;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

pub struct Store<'a>(pub &'a db::Store);
fn storage(error: db::StorageError) -> AuthError {
    match error {
        db::StorageError::Conflict => AuthError::Conflict("resource already exists"),
        error => AuthError::Internal(error.to_string()),
    }
}
fn d1(error: worker::Error) -> AuthError {
    storage(error.into())
}
fn decode<T: DeserializeOwned>(row: Value) -> AuthResult<T> {
    serde_json::from_value(row)
        .map_err(|e| AuthError::Internal(format!("invalid auth storage row: {e}")))
}
fn first<T: DeserializeOwned>(result: &worker::D1Result) -> AuthResult<Option<T>> {
    result
        .results::<Value>()
        .map_err(d1)?
        .into_iter()
        .next()
        .map(decode)
        .transpose()
}
fn date(now: DateTime<Utc>) -> String {
    time::format(now.timestamp_millis())
}
impl Store<'_> {
    async fn maybe<T: DeserializeOwned>(
        &self,
        sql: &str,
        values: Vec<Value>,
    ) -> AuthResult<Option<T>> {
        self.0
            .rows(sql, values)
            .await
            .map_err(storage)?
            .into_iter()
            .next()
            .map(decode)
            .transpose()
    }
}

impl AuthStore for Store<'_> {
    async fn has_accounts(&self) -> AuthResult<bool> {
        Ok(!self
            .0
            .rows("SELECT id FROM users LIMIT 1", vec![])
            .await
            .map_err(storage)?
            .is_empty())
    }
    async fn initialize_account_if_empty(&self, c: InitialAccountWrite) -> AuthResult<()> {
        self.0.run("INSERT INTO users (username,password_hash,created_at,updated_at) SELECT ?,?,?,? WHERE NOT EXISTS (SELECT 1 FROM users)",
            vec![json!(c.username),json!(c.password_hash),json!(date(c.now)),json!(date(c.now))]).await.map_err(storage)?;
        Ok(())
    }
    async fn account_by_username(&self, name: &str) -> AuthResult<Option<Account>> {
        self.maybe("SELECT * FROM users WHERE username=?", vec![json!(name)])
            .await
    }
    async fn account_by_id(&self, id: i64) -> AuthResult<Option<Account>> {
        self.maybe("SELECT * FROM users WHERE id=?", vec![json!(id)])
            .await
    }
    async fn first_account(&self) -> AuthResult<Option<Account>> {
        self.maybe(
            "SELECT * FROM users WHERE deleted_at IS NULL ORDER BY id LIMIT 1",
            vec![],
        )
        .await
    }
    async fn auth_state(&self, id: i64) -> AuthResult<Option<AuthState>> {
        // One read batch is a coherent snapshot; no auth filtering is hidden in
        // these reads. App decides expiry, ownership and revocation validity.
        let result=self.0.0.batch(vec![
            self.0.statement("SELECT * FROM sessions WHERE id=?",vec![json!(id)]).map_err(storage)?,
            self.0.statement("SELECT d.* FROM devices d JOIN sessions s ON s.device_id=d.id WHERE s.id=?",vec![json!(id)]).map_err(storage)?,
            self.0.statement("SELECT u.* FROM users u JOIN devices d ON d.user_id=u.id JOIN sessions s ON s.device_id=d.id WHERE s.id=?",vec![json!(id)]).map_err(storage)?,
        ]).await.map_err(d1)?;
        let (Some(session), Some(device), Some(account)) =
            (first(&result[0])?, first(&result[1])?, first(&result[2])?)
        else {
            return Ok(None);
        };
        Ok(Some(AuthState {
            account,
            device,
            session,
        }))
    }
    async fn complete_login(&self, c: LoginWrite) -> AuthResult<SessionGrant> {
        if c.max_devices == 0 {
            return Err(AuthError::Internal("invalid device limit".into()));
        }
        let now = date(c.now);
        let expires = date(c.expires_at);
        let d = c.device;
        let device=self.0.statement("INSERT INTO devices (user_id,device_id,name,platform,device_type,manufacturer,model,os_version,app_version,app_build,locale,time_zone,ip_address,user_agent,last_seen_at,created_at,updated_at) SELECT ?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,? FROM users WHERE id=? AND password_hash=? AND deleted_at IS NULL ON CONFLICT(user_id,device_id) DO UPDATE SET name=excluded.name,platform=excluded.platform,device_type=excluded.device_type,manufacturer=excluded.manufacturer,model=excluded.model,os_version=excluded.os_version,app_version=excluded.app_version,app_build=excluded.app_build,locale=excluded.locale,time_zone=excluded.time_zone,ip_address=excluded.ip_address,user_agent=excluded.user_agent,last_seen_at=excluded.last_seen_at,updated_at=excluded.updated_at,deleted_at=NULL",
            vec![json!(c.user_id),json!(d.device_id),json!(d.device_name),json!(d.platform),json!(d.device_type),json!(d.manufacturer),json!(d.model),json!(d.os_version),json!(d.app_version),json!(d.app_build),json!(d.locale),json!(d.time_zone),json!(c.meta.ip),json!(c.meta.user_agent),json!(now),json!(now),json!(now),json!(c.user_id),json!(c.expected_password_hash)]).map_err(storage)?;
        let session=self.0.statement("INSERT INTO sessions (device_id,refresh_token_hash,expires_at,logged_in_at,created_at,updated_at) SELECT d.id,?,?,?,?,? FROM devices d JOIN users u ON u.id=d.user_id WHERE d.user_id=? AND d.device_id=? AND u.password_hash=? AND u.deleted_at IS NULL ON CONFLICT(device_id) DO UPDATE SET refresh_token_hash=excluded.refresh_token_hash,expires_at=excluded.expires_at,logged_in_at=excluded.logged_in_at,updated_at=excluded.updated_at,revoked_at=NULL,deleted_at=NULL",
            vec![json!(c.refresh_token_hash),json!(expires),json!(now),json!(now),json!(now),json!(c.user_id),json!(d.device_id),json!(c.expected_password_hash)]).map_err(storage)?;
        let revoke=self.0.statement("UPDATE sessions SET revoked_at=?,updated_at=? WHERE id IN (SELECT s.id FROM sessions s JOIN devices d ON d.id=s.device_id WHERE d.user_id=? AND d.deleted_at IS NULL AND s.deleted_at IS NULL AND s.revoked_at IS NULL AND s.expires_at>? ORDER BY CASE WHEN s.refresh_token_hash=? THEN 0 ELSE 1 END,s.logged_in_at DESC,s.id DESC LIMIT -1 OFFSET ?) AND EXISTS (SELECT 1 FROM users WHERE id=? AND password_hash=? AND deleted_at IS NULL)",
            vec![json!(now),json!(now),json!(c.user_id),json!(now),json!(c.refresh_token_hash),json!(c.max_devices),json!(c.user_id),json!(c.expected_password_hash)]).map_err(storage)?;
        let result=self.0.0.batch(vec![device,session,revoke,
            self.0.statement("SELECT * FROM sessions WHERE refresh_token_hash=? AND revoked_at IS NULL AND deleted_at IS NULL",vec![json!(c.refresh_token_hash)]).map_err(storage)?,
            self.0.statement("SELECT u.* FROM users u JOIN devices d ON d.user_id=u.id JOIN sessions s ON s.device_id=d.id WHERE s.refresh_token_hash=? AND u.id=? AND u.password_hash=? AND u.deleted_at IS NULL",vec![json!(c.refresh_token_hash),json!(c.user_id),json!(c.expected_password_hash)]).map_err(storage)?,
        ]).await.map_err(d1)?;
        Ok(SessionGrant {
            session: first(&result[3])?.ok_or(AuthError::Unauthorized)?,
            account: first(&result[4])?.ok_or(AuthError::Unauthorized)?,
        })
    }
    async fn rotate_refresh_token(&self, c: RefreshWrite) -> AuthResult<SessionGrant> {
        let now = date(c.now);
        let result=self.0.0.batch(vec![
            self.0.statement("UPDATE sessions SET refresh_token_hash=?,expires_at=?,updated_at=? WHERE refresh_token_hash=? AND revoked_at IS NULL AND deleted_at IS NULL AND expires_at>? AND device_id IN (SELECT d.id FROM devices d JOIN users u ON u.id=d.user_id WHERE d.deleted_at IS NULL AND u.deleted_at IS NULL) RETURNING *",
                vec![json!(c.new_hash),json!(date(c.expires_at)),json!(now),json!(c.previous_hash),json!(now)]).map_err(storage)?,
            // changes() gates follow-up effects on the preceding atomic consume,
            // including the case where a failed consume's new hash already exists.
            self.0.statement("UPDATE devices SET ip_address=?,user_agent=?,last_seen_at=?,updated_at=? WHERE changes()=1 AND id IN (SELECT device_id FROM sessions WHERE refresh_token_hash=?)",
                vec![json!(c.meta.ip),json!(c.meta.user_agent),json!(now),json!(now),json!(c.new_hash)]).map_err(storage)?,
            self.0.statement("SELECT u.* FROM users u JOIN devices d ON d.user_id=u.id JOIN sessions s ON s.device_id=d.id WHERE s.refresh_token_hash=? AND changes()=1",vec![json!(c.new_hash)]).map_err(storage)?,
        ]).await.map_err(d1)?;
        Ok(SessionGrant {
            session: first(&result[0])?.ok_or(AuthError::Unauthorized)?,
            account: first(&result[2])?.ok_or(AuthError::Unauthorized)?,
        })
    }
    async fn revoke_refresh_token(&self, hash: &str, now: DateTime<Utc>) -> AuthResult<bool> {
        Ok(!self.0.rows("UPDATE sessions SET revoked_at=?,updated_at=? WHERE refresh_token_hash=? AND revoked_at IS NULL AND deleted_at IS NULL AND expires_at>? RETURNING id",
            vec![json!(date(now)),json!(date(now)),json!(hash),json!(date(now))]).await.map_err(storage)?.is_empty())
    }
    async fn update_account(&self, c: AccountWrite) -> AuthResult<Account> {
        let mut statements=vec![self.0.statement("UPDATE users SET username=?,password_hash=COALESCE(?,password_hash),updated_at=? WHERE id=? AND password_hash=? AND deleted_at IS NULL RETURNING *",
            vec![json!(c.username),json!(c.new_password_hash),json!(date(c.now)),json!(c.user_id),json!(c.expected_password_hash)]).map_err(storage)?];
        if c.new_password_hash.is_some() {
            statements.push(self.0.statement("UPDATE sessions SET revoked_at=?,updated_at=? WHERE changes()=1 AND device_id IN (SELECT id FROM devices WHERE user_id=?)",
                vec![json!(date(c.now)),json!(date(c.now)),json!(c.user_id)]).map_err(storage)?);
        }
        let result = self.0.0.batch(statements).await.map_err(d1)?;
        first(&result[0])?.ok_or(AuthError::Unauthorized)
    }
    async fn touch_device(
        &self,
        id: i64,
        before: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> AuthResult<()> {
        self.0.run("UPDATE devices SET last_seen_at=?,updated_at=? WHERE id=? AND (last_seen_at IS NULL OR last_seen_at<?)",
            vec![json!(date(now)),json!(date(now)),json!(id),json!(date(before))]).await.map_err(storage)?;
        Ok(())
    }
    async fn devices(&self, uid: i64) -> AuthResult<Vec<DeviceSession>> {
        let rows=self.0.rows("SELECT d.*,s.id AS auth_session_id,s.device_id AS auth_session_device_id,s.expires_at AS auth_session_expires_at,s.logged_in_at AS auth_session_logged_in_at,s.revoked_at AS auth_session_revoked_at,s.deleted_at AS auth_session_deleted_at FROM devices d LEFT JOIN sessions s ON s.device_id=d.id WHERE d.user_id=?",vec![json!(uid)]).await.map_err(storage)?;
        rows.into_iter().map(|row|{
            let session=if row["auth_session_id"].is_null(){None}else{Some(decode(json!({"id":row["auth_session_id"],"device_id":row["auth_session_device_id"],"expires_at":row["auth_session_expires_at"],"logged_in_at":row["auth_session_logged_in_at"],"revoked_at":row["auth_session_revoked_at"],"deleted_at":row["auth_session_deleted_at"]}))?)};
            Ok(DeviceSession{device:decode(row)?,session})
        }).collect()
    }
    async fn revoke_device_session(
        &self,
        uid: i64,
        identifier: &str,
        now: DateTime<Utc>,
    ) -> AuthResult<()> {
        let result=self.0.0.batch(vec![
            self.0.statement("SELECT id FROM devices WHERE user_id=? AND device_id=? AND deleted_at IS NULL",vec![json!(uid),json!(identifier)]).map_err(storage)?,
            self.0.statement("UPDATE sessions SET revoked_at=?,updated_at=? WHERE device_id IN (SELECT id FROM devices WHERE user_id=? AND device_id=? AND deleted_at IS NULL) AND revoked_at IS NULL AND deleted_at IS NULL",vec![json!(date(now)),json!(date(now)),json!(uid),json!(identifier)]).map_err(storage)?,
        ]).await.map_err(d1)?;
        if result[0].results::<Value>().map_err(d1)?.is_empty() {
            Err(AuthError::NotFound("device not found"))
        } else {
            Ok(())
        }
    }
    async fn delete_device(&self, uid: i64, identifier: &str) -> AuthResult<()> {
        // D1 FK cascade deletes the session in the same statement.
        if self.0.rows("DELETE FROM devices WHERE user_id=? AND device_id=? AND deleted_at IS NULL RETURNING id",vec![json!(uid),json!(identifier)]).await.map_err(storage)?.is_empty() {
            Err(AuthError::NotFound("device not found"))
        } else {Ok(())}
    }
}
