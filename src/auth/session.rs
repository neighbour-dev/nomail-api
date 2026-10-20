use super::{MAX_DEVICES, REFRESH_DAYS, access_token};
use crate::{
    AppState, crypto,
    entity::{device, session, user},
    error::{ApiError, ApiResult, check_length, success},
};
use actix_web::{HttpRequest, web};
use chrono::{Duration, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
    TransactionTrait,
};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
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

impl LoginRequest {
    fn validate(&self) -> Result<(), ApiError> {
        check_length(&self.username, "username", 64, true)?;
        check_length(&self.password, "password", usize::MAX, true)?;
        check_length(&self.device_id, "deviceId", 128, true)?;
        if ![
            "ios", "android", "macos", "windows", "linux", "web", "unknown",
        ]
        .contains(&self.platform.as_str())
        {
            return Err(ApiError::bad("unsupported platform"));
        }
        if !["phone", "tablet", "desktop", "web", "unknown"].contains(&self.device_type.as_str()) {
            return Err(ApiError::bad("unsupported deviceType"));
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

pub fn request_meta(req: &HttpRequest) -> (String, String) {
    // Do not trust forwarded headers unless a trusted proxy is explicitly implemented.
    let ip = req
        .peer_addr()
        .map(|a| a.ip().to_string())
        .unwrap_or_default();
    let agent = req
        .headers()
        .get("User-Agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .chars()
        .take(255)
        .collect();
    (ip, agent)
}

pub async fn login(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<LoginRequest>,
) -> ApiResult {
    body.validate()?;
    let account = user::Entity::find()
        .filter(user::Column::Username.eq(&body.username))
        .filter(user::Column::DeletedAt.is_null())
        .one(&state.db)
        .await?
        .ok_or_else(ApiError::unauthorized)?;
    crypto::verify_password(body.password.clone(), account.password_hash.clone()).await?;
    let tx = state.db.begin().await?;
    // Serialize logins per account so concurrent devices cannot bypass the device limit.
    let locked = user::Entity::find_by_id(account.id)
        .lock_exclusive()
        .one(&tx)
        .await?
        .ok_or_else(ApiError::unauthorized)?;
    if locked.password_hash != account.password_hash {
        return Err(ApiError::unauthorized());
    }
    let now = Utc::now();
    let existing = device::Entity::find()
        .filter(device::Column::UserId.eq(account.id))
        .filter(device::Column::DeviceId.eq(&body.device_id))
        .one(&tx)
        .await?;
    let is_new = existing.is_none();
    let mut active = existing
        .map(Into::into)
        .unwrap_or_else(|| device::ActiveModel {
            user_id: Set(account.id),
            device_id: Set(body.device_id.clone()),
            created_at: Set(now),
            ..Default::default()
        });
    let (ip, agent) = request_meta(&req);
    active.name = Set(Some(body.device_name.clone()));
    active.platform = Set(body.platform.clone());
    active.device_type = Set(body.device_type.clone());
    active.manufacturer = Set(Some(body.manufacturer.clone()));
    active.model = Set(Some(body.model.clone()));
    active.os_version = Set(Some(body.os_version.clone()));
    active.app_version = Set(Some(body.app_version.clone()));
    active.app_build = Set(Some(body.app_build.clone()));
    active.locale = Set(Some(body.locale.clone()));
    active.time_zone = Set(Some(body.time_zone.clone()));
    active.ip_address = Set(Some(ip));
    active.user_agent = Set(Some(agent));
    active.last_seen_at = Set(Some(now));
    active.updated_at = Set(now);
    active.deleted_at = Set(None);
    let device = if is_new {
        active.insert(&tx).await?
    } else {
        active.update(&tx).await?
    };
    let owned_devices = device::Entity::find()
        .filter(device::Column::UserId.eq(account.id))
        .filter(device::Column::DeletedAt.is_null())
        .all(&tx)
        .await?;
    let ids: Vec<i64> = owned_devices.iter().map(|d| d.id).collect();
    let others = session::Entity::find()
        .filter(session::Column::DeviceId.is_in(ids))
        .filter(session::Column::DeviceId.ne(device.id))
        .filter(session::Column::DeletedAt.is_null())
        .filter(session::Column::RevokedAt.is_null())
        .filter(session::Column::ExpiresAt.gt(now))
        .order_by_asc(session::Column::LoggedInAt)
        .all(&tx)
        .await?;
    let revoke_count = others.len().saturating_sub(MAX_DEVICES - 1);
    for old in others.into_iter().take(revoke_count) {
        let mut old: session::ActiveModel = old.into();
        old.revoked_at = Set(Some(now));
        old.updated_at = Set(now);
        old.update(&tx).await?;
    }
    let refresh_token = crypto::random_token(32);
    let refresh_expires = now + Duration::days(REFRESH_DAYS);
    let existing = session::Entity::find()
        .filter(session::Column::DeviceId.eq(device.id))
        .one(&tx)
        .await?;
    let is_new = existing.is_none();
    let mut active = existing
        .map(Into::into)
        .unwrap_or_else(|| session::ActiveModel {
            device_id: Set(device.id),
            created_at: Set(now),
            ..Default::default()
        });
    active.refresh_token_hash = Set(crypto::hash_token(&refresh_token));
    active.expires_at = Set(refresh_expires);
    active.logged_in_at = Set(now);
    active.revoked_at = Set(None);
    active.deleted_at = Set(None);
    active.updated_at = Set(now);
    let session = if is_new {
        active.insert(&tx).await?
    } else {
        active.update(&tx).await?
    };
    let (token, expires) = access_token(&state, account.id, session.id)?;
    tx.commit().await?;
    success_tokens(
        token,
        expires,
        refresh_token,
        refresh_expires,
        account.id,
        &account.username,
        body.username == state.config.initial_username
            && body.password == state.config.initial_password,
    )
}

fn success_tokens(
    access: String,
    access_expires: chrono::DateTime<Utc>,
    refresh: String,
    refresh_expires: chrono::DateTime<Utc>,
    user_id: i64,
    username: &str,
    setup: bool,
) -> ApiResult {
    Ok(success(
        json!({"accessToken": access, "accessTokenExpiresAt": access_expires,
        "refreshToken": refresh, "refreshTokenExpiresAt": refresh_expires,
        "user": {"id": user_id, "username": username, "requiresAccountSetup": setup}}),
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshRequest {
    pub refresh_token: String,
}

pub async fn refresh(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<RefreshRequest>,
) -> ApiResult {
    check_length(&body.refresh_token, "refreshToken", usize::MAX, true)?;
    let tx = state.db.begin().await?;
    let now = Utc::now();
    let session = session::Entity::find()
        .filter(session::Column::RefreshTokenHash.eq(crypto::hash_token(&body.refresh_token)))
        .filter(session::Column::DeletedAt.is_null())
        .filter(session::Column::RevokedAt.is_null())
        .filter(session::Column::ExpiresAt.gt(now))
        .lock_exclusive()
        .one(&tx)
        .await?
        .ok_or_else(ApiError::unauthorized)?;
    let device = device::Entity::find_by_id(session.device_id)
        .filter(device::Column::DeletedAt.is_null())
        .one(&tx)
        .await?
        .ok_or_else(ApiError::unauthorized)?;
    let account = user::Entity::find_by_id(device.user_id)
        .filter(user::Column::DeletedAt.is_null())
        .one(&tx)
        .await?
        .ok_or_else(ApiError::unauthorized)?;
    let refresh_token = crypto::random_token(32);
    let refresh_expires = now + Duration::days(REFRESH_DAYS);
    let (token, expires) = access_token(&state, account.id, session.id)?;
    let mut active: session::ActiveModel = session.into();
    active.refresh_token_hash = Set(crypto::hash_token(&refresh_token));
    active.expires_at = Set(refresh_expires);
    active.updated_at = Set(now);
    active.update(&tx).await?;
    let (ip, agent) = request_meta(&req);
    let mut active: device::ActiveModel = device.into();
    active.ip_address = Set(Some(ip));
    active.user_agent = Set(Some(agent));
    active.last_seen_at = Set(Some(now));
    active.updated_at = Set(now);
    active.update(&tx).await?;
    tx.commit().await?;
    success_tokens(
        token,
        expires,
        refresh_token,
        refresh_expires,
        account.id,
        &account.username,
        false,
    )
}

pub async fn logout(state: web::Data<AppState>, body: web::Json<RefreshRequest>) -> ApiResult {
    check_length(&body.refresh_token, "refreshToken", usize::MAX, true)?;
    let result = session::Entity::update_many()
        .col_expr(
            session::Column::RevokedAt,
            sea_orm::sea_query::Expr::value(Utc::now()),
        )
        .col_expr(
            session::Column::UpdatedAt,
            sea_orm::sea_query::Expr::value(Utc::now()),
        )
        .filter(session::Column::RefreshTokenHash.eq(crypto::hash_token(&body.refresh_token)))
        .filter(session::Column::DeletedAt.is_null())
        .filter(session::Column::RevokedAt.is_null())
        .filter(session::Column::ExpiresAt.gt(Utc::now()))
        .exec(&state.db)
        .await?;
    if result.rows_affected == 0 {
        return Err(ApiError::unauthorized());
    }
    Ok(success(json!(null)))
}
