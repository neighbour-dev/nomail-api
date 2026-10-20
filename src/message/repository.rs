use super::dto::{ListQuery, parse_bool, parse_time};
use crate::{
    entity::{message, webhook},
    error::ApiError,
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QuerySelect, Select,
    sea_query::{Expr, Query},
};
use sea_orm::{QueryTrait, sea_query::extension::postgres::PgExpr};
use uuid::Uuid;

pub fn owned(user_id: i64) -> Select<message::Entity> {
    let configs = Query::select()
        .column(webhook::Column::Id)
        .from(webhook::Entity)
        .and_where(webhook::Column::UserId.eq(user_id))
        .and_where(webhook::Column::DeletedAt.is_null())
        .to_owned();
    message::Entity::find()
        .filter(message::Column::DeletedAt.is_null())
        .filter(Expr::col(message::Column::WebhookConfigId).in_subquery(configs))
}

pub fn filtered(user_id: i64, q: &ListQuery) -> Result<Select<message::Entity>, ApiError> {
    let mut query = owned(user_id);
    if let Some(value) = q.query.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        // PostgreSQL ILIKE, preserving the existing API's substring search semantics.
        query = query.filter(
            Expr::col(message::Column::Title)
                .ilike(format!("%{value}%"))
                .or(Expr::col(message::Column::Content).ilike(format!("%{value}%"))),
        );
    }
    if let Some(value) = q.status.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        if !["read", "unread"].contains(&value) {
            return Err(ApiError::bad("status must be unread or read"));
        }
        query = query.filter(message::Column::Status.eq(value));
    }
    if let Some(value) = q.level.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        query = query.filter(message::Column::Level.eq(value));
    }
    if let Some(value) = q.webhook_config_id.as_deref().filter(|v| !v.is_empty()) {
        let id = value
            .parse::<i64>()
            .ok()
            .filter(|v| *v > 0)
            .ok_or_else(|| ApiError::bad("invalid webhookConfigId"))?;
        query = query.filter(message::Column::WebhookConfigId.eq(id));
    }
    if let Some(value) = q.pinned.as_deref().filter(|v| !v.is_empty()) {
        query = query.filter(message::Column::Pinned.eq(parse_bool(value)?));
    }
    let from = parse_time(q.from.as_deref(), "from")?;
    let to = parse_time(q.to.as_deref(), "to")?;
    if from.zip(to).is_some_and(|(a, b)| a > b) {
        return Err(ApiError::bad("from must not be after to"));
    }
    if let Some(from) = from {
        query = query.filter(message::Column::CreatedAt.gte(from));
    }
    if let Some(to) = to {
        query = query.filter(message::Column::CreatedAt.lte(to));
    }
    Ok(query)
}

pub async fn get(
    db: &impl ConnectionTrait,
    user_id: i64,
    id: Uuid,
) -> Result<message::Model, ApiError> {
    owned(user_id)
        .filter(message::Column::Id.eq(id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::not_found("message not found"))
}

pub fn owned_ids(user_id: i64) -> sea_orm::sea_query::SelectStatement {
    owned(user_id)
        .select_only()
        .column(message::Column::Id)
        .into_query()
}
