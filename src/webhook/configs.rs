use crate::{
    AppState,
    auth::Auth,
    crypto,
    entity::webhook,
    error::{ApiError, ApiResult, check_length, success},
};
use actix_web::web;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigRequest {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub secret: String,
    #[serde(default)]
    pub description: String,
}
impl ConfigRequest {
    fn validate(&self, required: bool) -> Result<(), ApiError> {
        check_length(&self.name, "name", 100, required)?;
        check_length(&self.provider, "provider", 32, required)?;
        check_length(&self.source, "source", 255, false)?;
        check_length(&self.secret, "secret", 500, false)?;
        check_length(&self.description, "description", 500, false)?;
        if required || !self.provider.is_empty() {
            super::parser::provider(&self.provider)?;
        }
        Ok(())
    }
}
pub fn response(c: webhook::Model) -> Value {
    json!({"id": c.id, "name": c.name, "key": c.key, "provider": c.provider,
        "source": c.source.unwrap_or_default(), "enabled": c.enabled,
        "description": c.description.unwrap_or_default(), "url": format!("/api/webhook/{}", c.key)})
}
pub async fn owned(
    db: &impl ConnectionTrait,
    user_id: i64,
    value: &str,
) -> Result<webhook::Model, ApiError> {
    let id = value
        .parse::<i64>()
        .ok()
        .filter(|v| *v > 0)
        .ok_or_else(|| ApiError::bad("invalid webhook config id"))?;
    webhook::Entity::find_by_id(id)
        .filter(webhook::Column::UserId.eq(user_id))
        .filter(webhook::Column::DeletedAt.is_null())
        .one(db)
        .await?
        .ok_or_else(|| ApiError::not_found("webhook config not found"))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListQuery {
    page: Option<String>,
    page_size: Option<String>,
}

pub async fn list(
    state: web::Data<AppState>,
    auth: Auth,
    query: web::Query<ListQuery>,
) -> ApiResult {
    let page = query
        .page
        .as_ref()
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(1) as u64;
    let size = query
        .page_size
        .as_ref()
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(20)
        .min(100) as u64;
    let query = webhook::Entity::find()
        .filter(webhook::Column::UserId.eq(auth.user_id))
        .filter(webhook::Column::DeletedAt.is_null());
    let total = query.clone().count(&state.db).await?;
    let items = query
        .order_by_desc(webhook::Column::CreatedAt)
        .offset((page - 1).saturating_mul(size))
        .limit(size)
        .all(&state.db)
        .await?;
    Ok(success(
        json!({"items": items.into_iter().map(response).collect::<Vec<_>>(), "total": total, "page": page, "pageSize": size}),
    ))
}
pub async fn create(
    state: web::Data<AppState>,
    auth: Auth,
    body: web::Json<ConfigRequest>,
) -> ApiResult {
    body.validate(true)?;
    let secret = if body.secret.is_empty() {
        String::new()
    } else {
        crypto::encrypt(&body.secret, &state.config.encryption_key)?
    };
    let now = Utc::now();
    let config = webhook::ActiveModel {
        user_id: Set(auth.user_id),
        name: Set(body.name.clone()),
        key: Set(crypto::random_token(16)),
        provider: Set(super::parser::provider(&body.provider)?.into()),
        source: Set(Some(body.source.clone())),
        encrypted_secret: Set(Some(secret)),
        enabled: Set(true),
        description: Set(Some(body.description.clone())),
        created_at: Set(now),
        updated_at: Set(now),
        deleted_at: Set(None),
        ..Default::default()
    }
    .insert(&state.db)
    .await?;
    Ok(success(response(config)))
}
pub async fn get(state: web::Data<AppState>, auth: Auth, path: web::Path<String>) -> ApiResult {
    Ok(success(response(
        owned(&state.db, auth.user_id, &path).await?,
    )))
}
pub async fn update(
    state: web::Data<AppState>,
    auth: Auth,
    path: web::Path<String>,
    body: web::Json<ConfigRequest>,
) -> ApiResult {
    body.validate(false)?;
    let config = owned(&state.db, auth.user_id, &path).await?;
    // Preserve existing PATCH semantics: empty strings are ignored.
    if body.name.is_empty()
        && body.provider.is_empty()
        && body.source.is_empty()
        && body.secret.is_empty()
        && body.description.is_empty()
    {
        return Ok(success(response(config)));
    }
    let mut active: webhook::ActiveModel = config.into();
    if !body.name.is_empty() {
        active.name = Set(body.name.clone());
    }
    if !body.provider.is_empty() {
        active.provider = Set(super::parser::provider(&body.provider)?.into());
    }
    if !body.source.is_empty() {
        active.source = Set(Some(body.source.clone()));
    }
    if !body.secret.is_empty() {
        active.encrypted_secret = Set(Some(crypto::encrypt(
            &body.secret,
            &state.config.encryption_key,
        )?));
    }
    if !body.description.is_empty() {
        active.description = Set(Some(body.description.clone()));
    }
    active.updated_at = Set(Utc::now());
    Ok(success(response(active.update(&state.db).await?)))
}
#[derive(Deserialize)]
pub struct EnabledRequest {
    enabled: bool,
}
pub async fn enabled(
    state: web::Data<AppState>,
    auth: Auth,
    path: web::Path<String>,
    body: web::Json<EnabledRequest>,
) -> ApiResult {
    let config = owned(&state.db, auth.user_id, &path).await?;
    let mut active: webhook::ActiveModel = config.into();
    active.enabled = Set(body.enabled);
    active.updated_at = Set(Utc::now());
    Ok(success(response(active.update(&state.db).await?)))
}
pub async fn delete(state: web::Data<AppState>, auth: Auth, path: web::Path<String>) -> ApiResult {
    let config = owned(&state.db, auth.user_id, &path).await?;
    let mut active: webhook::ActiveModel = config.into();
    active.deleted_at = Set(Some(Utc::now()));
    active.updated_at = Set(Utc::now());
    active.update(&state.db).await?;
    Ok(success(json!(null)))
}
