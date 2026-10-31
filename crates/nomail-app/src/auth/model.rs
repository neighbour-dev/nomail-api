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

#[derive(Clone, Deserialize)]
pub struct Device {
    pub id: i64,
    pub user_id: i64,
    pub device_id: String,
    pub name: Option<String>,
    pub platform: String,
    pub device_type: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub os_version: Option<String>,
    pub app_version: Option<String>,
    pub app_build: Option<String>,
    pub locale: Option<String>,
    pub time_zone: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

pub struct DeviceSession {
    pub device: Device,
    pub session: Option<Session>,
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

/// Only device registration data crosses the storage boundary, never the
/// plaintext login password or plaintext refresh token.
#[derive(Default)]
pub struct DeviceInput {
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
    pub device_type: String,
    pub manufacturer: String,
    pub model: String,
    pub os_version: String,
    pub app_version: String,
    pub app_build: String,
    pub locale: String,
    pub time_zone: String,
}
