use super::Auth;
use crate::{
    AppState,
    config::Config,
    crypto,
    entity::{device, session, user},
    error::{ApiError, ApiResult, check_length, success},
};
use actix_web::web;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait, sea_query::Expr,
};
use serde::Deserialize;
use serde_json::json;

pub async fn ensure_initial_user(
    db: &sea_orm::DatabaseConnection,
    config: &Config,
) -> Result<(), ApiError> {
    let tx = db.begin().await?;
    // Protect first-run initialization across concurrent server instances.
    tx.execute_unprepared("SELECT pg_advisory_xact_lock(716209458)")
        .await?;
    if user::Entity::find()
        .filter(user::Column::DeletedAt.is_null())
        .count(&tx)
        .await?
        == 0
    {
        check_length(&config.initial_username, "INITIAL_USERNAME", 64, true)?;
        crypto::validate_password(&config.initial_password)?;
        let hash = crypto::hash_password(config.initial_password.clone()).await?;
        let now = Utc::now();
        user::ActiveModel {
            username: Set(config.initial_username.clone()),
            password_hash: Set(hash),
            created_at: Set(now),
            updated_at: Set(now),
            deleted_at: Set(None),
            ..Default::default()
        }
        .insert(&tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn get(state: web::Data<AppState>, auth: Auth) -> ApiResult {
    let user = user::Entity::find_by_id(auth.user_id)
        .filter(user::Column::DeletedAt.is_null())
        .one(&state.db)
        .await?
        .ok_or_else(ApiError::unauthorized)?;
    Ok(success(
        json!({"id": user.id, "username": user.username, "requiresAccountSetup": false}),
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRequest {
    pub current_password: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub new_password: String,
}

pub async fn update(
    state: web::Data<AppState>,
    auth: Auth,
    body: web::Json<UpdateRequest>,
) -> ApiResult {
    check_length(&body.current_password, "currentPassword", usize::MAX, true)?;
    check_length(&body.username, "username", 64, false)?;
    if body.username.is_empty() && body.new_password.is_empty() {
        return Err(ApiError::bad("username or newPassword is required"));
    }
    if !body.new_password.is_empty() {
        crypto::validate_password(&body.new_password)?;
    }
    let tx = state.db.begin().await?;
    let account = user::Entity::find_by_id(auth.user_id)
        .filter(user::Column::DeletedAt.is_null())
        .lock_exclusive()
        .one(&tx)
        .await?
        .ok_or_else(ApiError::unauthorized)?;
    crypto::verify_password(body.current_password.clone(), account.password_hash.clone()).await?;
    let username = if body.username.is_empty() {
        account.username.clone()
    } else {
        body.username.clone()
    };
    let mut active: user::ActiveModel = account.into();
    active.username = Set(username.clone());
    active.updated_at = Set(Utc::now());
    if !body.new_password.is_empty() {
        active.password_hash = Set(crypto::hash_password(body.new_password.clone()).await?);
        revoke_sessions(&tx, auth.user_id).await?;
    }
    active.update(&tx).await?;
    tx.commit().await?;
    Ok(success(
        json!({"id": auth.user_id, "username": username, "requiresAccountSetup": false}),
    ))
}

pub async fn revoke_sessions(db: &impl ConnectionTrait, user_id: i64) -> Result<(), ApiError> {
    let devices = device::Entity::find()
        .filter(device::Column::UserId.eq(user_id))
        .filter(device::Column::DeletedAt.is_null())
        .all(db)
        .await?;
    session::Entity::update_many()
        .col_expr(session::Column::RevokedAt, Expr::value(Utc::now()))
        .col_expr(session::Column::UpdatedAt, Expr::value(Utc::now()))
        .filter(session::Column::DeviceId.is_in(devices.into_iter().map(|d| d.id)))
        .filter(session::Column::RevokedAt.is_null())
        .filter(session::Column::DeletedAt.is_null())
        .exec(db)
        .await?;
    Ok(())
}

pub async fn reset_account(
    db: &sea_orm::DatabaseConnection,
    username: Option<String>,
    password: String,
) -> Result<(), ApiError> {
    crypto::validate_password(&password)?;
    if let Some(name) = &username {
        check_length(name, "username", 64, true)?;
    }
    let hash = crypto::hash_password(password).await?;
    let tx = db.begin().await?;
    let account = user::Entity::find()
        .filter(user::Column::DeletedAt.is_null())
        .order_by_asc(user::Column::Id)
        .lock_exclusive()
        .one(&tx)
        .await?
        .ok_or_else(|| ApiError::not_found("account not found"))?;
    let id = account.id;
    let mut active: user::ActiveModel = account.into();
    if let Some(name) = username {
        active.username = Set(name);
    }
    active.password_hash = Set(hash);
    active.updated_at = Set(Utc::now());
    active.update(&tx).await?;
    revoke_sessions(&tx, id).await?;
    tx.commit().await?;
    Ok(())
}
