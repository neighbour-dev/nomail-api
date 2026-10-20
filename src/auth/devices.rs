use super::Auth;
use crate::{
    AppState,
    entity::{device, push_endpoint, session},
    error::{ApiError, ApiResult, check_length, success},
};
use actix_web::web;
use chrono::Utc;
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, TransactionTrait,
    sea_query::Expr,
};
use serde_json::json;

pub async fn owned(
    db: &impl ConnectionTrait,
    user_id: i64,
    identifier: &str,
) -> Result<device::Model, ApiError> {
    check_length(identifier, "deviceId", 128, true)?;
    device::Entity::find()
        .filter(device::Column::UserId.eq(user_id))
        .filter(device::Column::DeviceId.eq(identifier))
        .filter(device::Column::DeletedAt.is_null())
        .one(db)
        .await?
        .ok_or_else(|| ApiError::not_found("device not found"))
}

pub async fn list(state: web::Data<AppState>, auth: Auth) -> ApiResult {
    let devices = device::Entity::find()
        .filter(device::Column::UserId.eq(auth.user_id))
        .filter(device::Column::DeletedAt.is_null())
        .order_by_desc(device::Column::UpdatedAt)
        .all(&state.db)
        .await?;
    let ids: Vec<i64> = devices.iter().map(|d| d.id).collect();
    let sessions = session::Entity::find()
        .filter(session::Column::DeviceId.is_in(ids.clone()))
        .filter(session::Column::DeletedAt.is_null())
        .all(&state.db)
        .await?;
    let endpoints = push_endpoint::Entity::find()
        .filter(push_endpoint::Column::DeviceId.is_in(ids))
        .filter(push_endpoint::Column::DeletedAt.is_null())
        .all(&state.db)
        .await?;
    let now = Utc::now();
    let mut items = Vec::new();
    for d in devices {
        let session = sessions.iter().find(|s| s.device_id == d.id);
        let logged_in = session.is_some_and(|s| s.revoked_at.is_none() && s.expires_at > now);
        let push_enabled = endpoints
            .iter()
            .any(|p| p.device_id == d.id && p.enabled && p.invalidated_at.is_none());
        items.push(json!({"deviceId": d.device_id, "deviceName": d.name.unwrap_or_default(),
            "platform": d.platform, "deviceType": d.device_type, "manufacturer": d.manufacturer.unwrap_or_default(),
            "model": d.model.unwrap_or_default(), "osVersion": d.os_version.unwrap_or_default(),
            "appVersion": d.app_version.unwrap_or_default(), "appBuild": d.app_build.unwrap_or_default(),
            "locale": d.locale.unwrap_or_default(), "timeZone": d.time_zone.unwrap_or_default(),
            "ipAddress": d.ip_address.unwrap_or_default(), "userAgent": d.user_agent.unwrap_or_default(),
            "lastSeenAt": d.last_seen_at, "loggedIn": logged_in, "pushEnabled": push_enabled,
            "expiresAt": session.map(|s| s.expires_at), "loggedInAt": session.map(|s| s.logged_in_at),
            "createdAt": d.created_at, "updatedAt": d.updated_at}));
    }
    // Stable sort retains updated_at ordering within each login-state group.
    items.sort_by_key(|v| !v["loggedIn"].as_bool().unwrap_or(false));
    Ok(success(items))
}

pub async fn kick(state: web::Data<AppState>, auth: Auth, path: web::Path<String>) -> ApiResult {
    let device = owned(&state.db, auth.user_id, &path).await?;
    session::Entity::update_many()
        .col_expr(session::Column::RevokedAt, Expr::value(Utc::now()))
        .col_expr(session::Column::UpdatedAt, Expr::value(Utc::now()))
        .filter(session::Column::DeviceId.eq(device.id))
        .filter(session::Column::DeletedAt.is_null())
        .filter(session::Column::RevokedAt.is_null())
        .exec(&state.db)
        .await?;
    Ok(success(json!(null)))
}

pub async fn delete(state: web::Data<AppState>, auth: Auth, path: web::Path<String>) -> ApiResult {
    let tx = state.db.begin().await?;
    let device = owned(&tx, auth.user_id, &path).await?;
    push_endpoint::Entity::delete_many()
        .filter(push_endpoint::Column::DeviceId.eq(device.id))
        .exec(&tx)
        .await?;
    session::Entity::delete_many()
        .filter(session::Column::DeviceId.eq(device.id))
        .exec(&tx)
        .await?;
    device::Entity::delete_by_id(device.id).exec(&tx).await?;
    tx.commit().await?;
    Ok(success(json!(null)))
}
