use crate::auth::Session;
use chrono::{DateTime, Utc};
use serde::Deserialize;

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

pub struct DeviceInfo {
    pub device: Device,
    pub session: Option<Session>,
    pub logged_in: bool,
}

/// Device metadata attached to a login; never contains credentials.
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
