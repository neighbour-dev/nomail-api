//! Account DTOs and validation, shared by server and Workers.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub user_id: i64,
    pub session_id: i64,
    pub sub: String,
    pub iss: String,
    pub iat: i64,
    pub exp: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    pub device_id: String,
    pub platform: String,
    pub device_type: String,
    #[serde(default)]
    pub device_name: String,
    #[serde(default)]
    pub manufacturer: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub os_version: String,
    #[serde(default)]
    pub app_version: String,
    #[serde(default)]
    pub app_build: String,
    #[serde(default)]
    pub locale: String,
    #[serde(default)]
    pub time_zone: String,
}
pub fn check_length(value: &str, name: &str, max: usize, required: bool) -> Result<(), String> {
    if (required && value.is_empty()) || value.chars().count() > max {
        Err(format!("invalid {name}"))
    } else {
        Ok(())
    }
}
impl LoginRequest {
    pub fn validate(&self) -> Result<(), String> {
        check_length(&self.username, "username", 64, true)?;
        check_length(&self.password, "password", usize::MAX, true)?;
        check_length(&self.device_id, "deviceId", 128, true)?;
        if ![
            "ios", "android", "macos", "windows", "linux", "web", "unknown",
        ]
        .contains(&self.platform.as_str())
        {
            return Err("unsupported platform".into());
        }
        if !["phone", "tablet", "desktop", "web", "unknown"].contains(&self.device_type.as_str()) {
            return Err("unsupported deviceType".into());
        }
        for (name, value, max) in [
            ("deviceName", &self.device_name, 128),
            ("manufacturer", &self.manufacturer, 128),
            ("model", &self.model, 128),
            ("osVersion", &self.os_version, 64),
            ("appVersion", &self.app_version, 64),
            ("appBuild", &self.app_build, 64),
            ("locale", &self.locale, 32),
            ("timeZone", &self.time_zone, 64),
        ] {
            check_length(value, name, max, false)?;
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct AccountUpdateRequest {
    pub current_password: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub new_password: String,
}

impl AccountUpdateRequest {
    pub fn validate(&self) -> Result<(), String> {
        check_length(&self.current_password, "currentPassword", usize::MAX, true)?;
        check_length(&self.username, "username", 64, false)?;
        if self.username.is_empty() && self.new_password.is_empty() {
            return Err("username or newPassword is required".into());
        }
        if !self.new_password.is_empty() {
            crate::security::validate_password(&self.new_password).map_err(String::from)?;
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct AccountResponse {
    pub id: i64,
    pub username: String,
    pub requires_account_setup: bool,
}

/// Deliberately not Debug: contains bearer credentials.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct TokenResponse {
    pub access_token: String,
    pub access_token_expires_at: DateTime<Utc>,
    pub refresh_token: String,
    pub refresh_token_expires_at: DateTime<Utc>,
    pub user: AccountResponse,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct DeviceResponse {
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
    pub ip_address: String,
    pub user_agent: String,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub logged_in: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub logged_in_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
