use crate::device::Device;
use chrono::{DateTime, Utc};
use serde::Deserialize;

/// Database account snapshot. Password hashes never enter response DTOs/logs.
#[derive(Clone, Deserialize)]
pub struct Account {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Deserialize)]
pub struct Session {
    pub id: i64,
    pub device_id: i64,
    pub expires_at: DateTime<Utc>,
    pub logged_in_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
}
impl Session {
    pub fn active(&self, now: DateTime<Utc>) -> bool {
        self.deleted_at.is_none() && self.revoked_at.is_none() && self.expires_at > now
    }
}

pub struct AuthState {
    pub account: Account,
    pub device: Device,
    pub session: Session,
}
pub struct SessionGrant {
    pub account: Account,
    pub session: Session,
}

pub struct AuthContext {
    pub user_id: i64,
    pub session_id: i64,
    pub device_id: i64,
    pub logged_in_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, Default)]
pub struct RequestMeta {
    pub ip: String,
    pub user_agent: String,
}
impl RequestMeta {
    pub fn new(ip: String, user_agent: String) -> Self {
        Self {
            ip: ip.chars().take(64).collect(),
            user_agent: user_agent.chars().take(255).collect(),
        }
    }
}

/// Public account details returned by authentication use cases.
pub struct AccountInfo {
    pub id: i64,
    pub username: String,
}

/// Login and refresh result. Deliberately not Debug/Serialize: contains bearer credentials.
pub struct SessionTokens {
    pub access_token: String,
    pub access_token_expires_at: DateTime<Utc>,
    pub refresh_token: String,
    pub refresh_token_expires_at: DateTime<Utc>,
    pub user: AccountInfo,
}
