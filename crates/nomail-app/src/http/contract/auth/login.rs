use super::endpoint;
use super::get_account::AccountResponse;
use crate::http::{Environment, HttpResult, dispatch::EndpointCall};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

endpoint! {
    Login {
        method: Post,
        path: "/api/auth/login",
        auth: Public,
        operation_id: "login",
        tag: "Auth",
        summary: "登录",
        request: LoginRequest,
        query: (),
        response: TokenResponse,
        body_kind: Json,
        response_kind: Json,
        has_query: false,
        path_kind: None,
    }
}

impl Login {
    pub(in crate::http) async fn handle(
        call: EndpointCall<'_, impl Environment>,
    ) -> HttpResult<TokenResponse> {
        Ok(call
            .services
            .auth
            .login(call.input::<Self>()?.into(), call.head.metadata.clone())
            .await?
            .into())
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
