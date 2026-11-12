use super::{Device, Session};
use chrono::{DateTime, Utc};

pub struct AccountInfo {
    pub id: i64,
    pub username: String,
}
/// Deliberately not Debug/Serialize: contains bearer credentials.
pub struct SessionTokens {
    pub access_token: String,
    pub access_token_expires_at: DateTime<Utc>,
    pub refresh_token: String,
    pub refresh_token_expires_at: DateTime<Utc>,
    pub user: AccountInfo,
}
pub struct DeviceInfo {
    pub device: Device,
    pub session: Option<Session>,
    pub logged_in: bool,
}
