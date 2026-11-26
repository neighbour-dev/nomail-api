//! Authentication request/response JSON shapes and application conversions.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct SetupRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct SetupStatusResponse {
    pub initialized: bool,
}

impl From<SetupRequest> for crate::auth::InitializeAccount {
    fn from(request: SetupRequest) -> Self {
        Self {
            username: request.username,
            password: request.password,
        }
    }
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

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct AccountResponse {
    pub id: i64,
    pub username: String,
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

impl From<LoginRequest> for crate::auth::Login {
    fn from(request: LoginRequest) -> Self {
        Self {
            username: request.username,
            password: request.password,
            device: crate::device::DeviceInput {
                device_id: request.device_id,
                device_name: request.device_name,
                platform: request.platform,
                device_type: request.device_type,
                manufacturer: request.manufacturer,
                model: request.model,
                os_version: request.os_version,
                app_version: request.app_version,
                app_build: request.app_build,
                locale: request.locale,
                time_zone: request.time_zone,
            },
        }
    }
}
impl From<AccountUpdateRequest> for crate::auth::UpdateAccount {
    fn from(request: AccountUpdateRequest) -> Self {
        Self {
            current_password: request.current_password,
            username: request.username,
            new_password: request.new_password,
        }
    }
}
impl From<crate::auth::AccountInfo> for AccountResponse {
    fn from(account: crate::auth::AccountInfo) -> Self {
        Self {
            id: account.id,
            username: account.username,
        }
    }
}
impl From<crate::auth::SessionTokens> for TokenResponse {
    fn from(tokens: crate::auth::SessionTokens) -> Self {
        Self {
            access_token: tokens.access_token,
            access_token_expires_at: tokens.access_token_expires_at,
            refresh_token: tokens.refresh_token,
            refresh_token_expires_at: tokens.refresh_token_expires_at,
            user: tokens.user.into(),
        }
    }
}
