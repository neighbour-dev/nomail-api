use super::{
    Account, AuthResult, AuthState, DeviceInput, DeviceSession, RequestMeta, SessionGrant,
};
use chrono::{DateTime, Utc};

pub struct InitialAccountWrite {
    pub username: String,
    pub password_hash: String,
    pub now: DateTime<Utc>,
}
pub struct LoginWrite {
    pub user_id: i64,
    pub expected_password_hash: String,
    pub device: DeviceInput,
    pub meta: RequestMeta,
    pub refresh_token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub now: DateTime<Utc>,
    pub max_devices: usize,
}
pub struct RefreshWrite {
    pub previous_hash: String,
    pub new_hash: String,
    pub expires_at: DateTime<Utc>,
    pub now: DateTime<Utc>,
    pub meta: RequestMeta,
}
pub struct AccountWrite {
    pub user_id: i64,
    pub expected_password_hash: String,
    pub username: String,
    pub new_password_hash: Option<String>,
    pub now: DateTime<Utc>,
}

/// Atomic operation contract, not a generic ORM. No HTTP or runtime types.
/// D1 futures need not be Send; callers use static dispatch on both platforms.
#[allow(async_fn_in_trait)]
pub trait AuthStore {
    async fn has_accounts(&self) -> AuthResult<bool>;
    async fn initialize_account_if_empty(&self, command: InitialAccountWrite) -> AuthResult<()>;
    async fn account_by_username(&self, username: &str) -> AuthResult<Option<Account>>;
    async fn account_by_id(&self, id: i64) -> AuthResult<Option<Account>>;
    async fn first_account(&self) -> AuthResult<Option<Account>>;
    /// Coherent account/device/session snapshot, including revocation/deletion.
    async fn auth_state(&self, session_id: i64) -> AuthResult<Option<AuthState>>;
    /// Recheck account credentials, upsert device/session, enforce max_devices in
    /// one transaction. Keep the current device, then newest (logged_in_at,id).
    async fn complete_login(&self, command: LoginWrite) -> AuthResult<SessionGrant>;
    /// Consume the old hash exactly once. Check account/device/session validity
    /// and update device IP, user agent and last_seen_at in the SAME transaction.
    async fn rotate_refresh_token(&self, command: RefreshWrite) -> AuthResult<SessionGrant>;
    async fn revoke_refresh_token(&self, hash: &str, now: DateTime<Utc>) -> AuthResult<bool>;
    /// Compare credentials again and update account. A new password hash must
    /// atomically revoke ALL of the user's sessions, including expired sessions.
    async fn update_account(&self, command: AccountWrite) -> AuthResult<Account>;
    async fn touch_device(
        &self,
        id: i64,
        before: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> AuthResult<()>;
    async fn devices(&self, user_id: i64) -> AuthResult<Vec<DeviceSession>>;
    async fn revoke_device_session(
        &self,
        user_id: i64,
        identifier: &str,
        now: DateTime<Utc>,
    ) -> AuthResult<()>;
    async fn delete_device(&self, user_id: i64, identifier: &str) -> AuthResult<()>;
}
