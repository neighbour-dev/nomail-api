use crate::{
    AppState,
    auth::{Auth, devices::owned},
    crypto,
    entity::push_endpoint as endpoint,
    error::{ApiError, ApiResult, check_length, success},
};
use actix_web::web;
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterRequest {
    pub device_id: String,
    pub provider: String,
    #[serde(default)]
    pub environment: String,
    pub push_token: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRequest {
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub environment: String,
    #[serde(default)]
    pub push_token: String,
    pub enabled: Option<bool>,
}

fn validate(
    provider: &str,
    environment: &str,
    token: &str,
    required: bool,
) -> Result<(), ApiError> {
    if (required || !provider.is_empty()) && !["apns", "fcm"].contains(&provider) {
        return Err(ApiError::bad("unsupported provider"));
    }
    if !environment.is_empty() && !["sandbox", "production"].contains(&environment) {
        return Err(ApiError::bad("unsupported environment"));
    }
    check_length(token, "pushToken", 8192, required)
}
fn response(identifier: &str, endpoint: endpoint::Model) -> Value {
    json!({"deviceId": identifier, "provider": endpoint.provider, "environment": endpoint.environment,
        "enabled": endpoint.enabled, "registeredAt": endpoint.registered_at,
        "invalidatedAt": endpoint.invalidated_at, "lastError": endpoint.last_error.unwrap_or_default()})
}

pub async fn register(
    state: web::Data<AppState>,
    auth: Auth,
    body: web::Json<RegisterRequest>,
) -> ApiResult {
    validate(&body.provider, &body.environment, &body.push_token, true)?;
    let encrypted = crypto::encrypt(&body.push_token, &state.config.encryption_key)?;
    let tx = state.db.begin().await?;
    // The device row serializes registrations for the same device.
    use sea_orm::QuerySelect;
    let device = owned(&tx, auth.user_id, &body.device_id).await?;
    crate::entity::device::Entity::find_by_id(device.id)
        .lock_exclusive()
        .one(&tx)
        .await?;
    let existing = endpoint::Entity::find()
        .filter(endpoint::Column::DeviceId.eq(device.id))
        .one(&tx)
        .await?;
    let is_new = existing.is_none();
    let now = Utc::now();
    let mut active = existing
        .map(Into::into)
        .unwrap_or_else(|| endpoint::ActiveModel {
            device_id: Set(device.id),
            created_at: Set(now),
            ..Default::default()
        });
    active.provider = Set(body.provider.clone());
    active.environment = Set(if body.environment.is_empty() {
        "production".into()
    } else {
        body.environment.clone()
    });
    active.encrypted_token = Set(encrypted);
    active.token_hash = Set(crypto::hash_token(&body.push_token));
    active.enabled = Set(true);
    active.registered_at = Set(now);
    active.invalidated_at = Set(None);
    active.last_error = Set(Some(String::new()));
    active.updated_at = Set(now);
    active.deleted_at = Set(None);
    let endpoint = if is_new {
        active.insert(&tx).await?
    } else {
        active.update(&tx).await?
    };
    tx.commit().await?;
    Ok(success(response(&body.device_id, endpoint)))
}

pub async fn update(
    state: web::Data<AppState>,
    auth: Auth,
    path: web::Path<String>,
    body: web::Json<UpdateRequest>,
) -> ApiResult {
    validate(&body.provider, &body.environment, &body.push_token, false)?;
    if body.provider.is_empty()
        && body.environment.is_empty()
        && body.push_token.is_empty()
        && body.enabled.is_none()
    {
        return Err(ApiError::bad("at least one field is required"));
    }
    let device = owned(&state.db, auth.user_id, &path).await?;
    let endpoint = endpoint::Entity::find()
        .filter(endpoint::Column::DeviceId.eq(device.id))
        .filter(endpoint::Column::DeletedAt.is_null())
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::not_found("push endpoint not found"))?;
    let mut active: endpoint::ActiveModel = endpoint.into();
    if !body.provider.is_empty() {
        active.provider = Set(body.provider.clone());
    }
    if !body.environment.is_empty() {
        active.environment = Set(body.environment.clone());
    }
    if let Some(enabled) = body.enabled {
        active.enabled = Set(enabled);
    }
    if !body.push_token.is_empty() {
        active.encrypted_token = Set(crypto::encrypt(
            &body.push_token,
            &state.config.encryption_key,
        )?);
        active.token_hash = Set(crypto::hash_token(&body.push_token));
        active.registered_at = Set(Utc::now());
        active.invalidated_at = Set(None);
        active.last_error = Set(Some(String::new()));
    }
    active.updated_at = Set(Utc::now());
    Ok(success(response(&path, active.update(&state.db).await?)))
}

pub async fn delete(state: web::Data<AppState>, auth: Auth, path: web::Path<String>) -> ApiResult {
    let device = owned(&state.db, auth.user_id, &path).await?;
    let result = endpoint::Entity::delete_many()
        .filter(endpoint::Column::DeviceId.eq(device.id))
        .exec(&state.db)
        .await?;
    if result.rows_affected == 0 {
        return Err(ApiError::not_found("push endpoint not found"));
    }
    Ok(success(json!(null)))
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/push/devices")
            .route("", web::post().to(register))
            .route("/{device_id}", web::patch().to(update))
            .route("/{device_id}", web::delete().to(delete)),
    );
}
