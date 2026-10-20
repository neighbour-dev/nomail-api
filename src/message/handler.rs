use super::{
    dto::{self, BatchRequest, ListQuery, UpdateRequest},
    repository,
};
use crate::{
    AppState,
    auth::Auth,
    entity::{message, webhook},
    error::{ApiError, ApiResult, success},
    realtime::{Event, encode},
};
use actix_web::{HttpResponse, web};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Set, sea_query::Expr,
};
use serde_json::json;

pub async fn list(
    state: web::Data<AppState>,
    auth: Auth,
    query: web::Query<ListQuery>,
) -> ApiResult {
    let (page, size) = dto::pagination(query.page.as_deref(), query.page_size.as_deref());
    let query = repository::filtered(auth.user_id, &query)?;
    let total = query.clone().count(&state.db).await?;
    let items = query
        .order_by_desc(message::Column::CreatedAt)
        .offset((page - 1).saturating_mul(size))
        .limit(size)
        .all(&state.db)
        .await?;
    Ok(success(
        json!({"items": items.iter().map(dto::item).collect::<Vec<_>>(), "total": total, "page": page, "pageSize": size}),
    ))
}
pub async fn unread_count(state: web::Data<AppState>, auth: Auth) -> ApiResult {
    let count = repository::owned(auth.user_id)
        .filter(message::Column::Status.eq("unread"))
        .count(&state.db)
        .await?;
    Ok(success(json!({"count": count})))
}
async fn detail(state: &AppState, message: &message::Model) -> Result<serde_json::Value, ApiError> {
    let config = webhook::Entity::find_by_id(message.webhook_config_id)
        .filter(webhook::Column::DeletedAt.is_null())
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::not_found("webhook config not found"))?;
    Ok(dto::detail(message, &config))
}
pub async fn get(state: web::Data<AppState>, auth: Auth, path: web::Path<String>) -> ApiResult {
    let message = repository::get(&state.db, auth.user_id, dto::parse_id(&path)?).await?;
    Ok(success(detail(&state, &message).await?))
}
pub async fn update(
    state: web::Data<AppState>,
    auth: Auth,
    path: web::Path<String>,
    body: web::Json<UpdateRequest>,
) -> ApiResult {
    body.validate()?;
    let message = repository::get(&state.db, auth.user_id, dto::parse_id(&path)?).await?;
    let mut active: message::ActiveModel = message.into();
    if let Some(status) = &body.status {
        active.status = Set(status.clone());
        active.read_at = Set(if status == "read" {
            Some(Utc::now())
        } else {
            None
        });
    }
    if let Some(pinned) = body.pinned {
        active.pinned = Set(Some(pinned));
    }
    active.updated_at = Set(Utc::now());
    let updated = active.update(&state.db).await?;
    Ok(success(detail(&state, &updated).await?))
}
pub async fn delete(state: web::Data<AppState>, auth: Auth, path: web::Path<String>) -> ApiResult {
    let message = repository::get(&state.db, auth.user_id, dto::parse_id(&path)?).await?;
    let mut active: message::ActiveModel = message.into();
    active.deleted_at = Set(Some(Utc::now()));
    active.updated_at = Set(Utc::now());
    active.update(&state.db).await?;
    Ok(success(json!(null)))
}
pub async fn raw(state: web::Data<AppState>, auth: Auth, path: web::Path<String>) -> ApiResult {
    let message = repository::get(&state.db, auth.user_id, dto::parse_id(&path)?).await?;
    let payload = message
        .raw_payload
        .filter(|v| !v.is_empty())
        .ok_or_else(|| ApiError::not_found("raw payload not found"))?;
    let content_type = message
        .content_type
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "application/octet-stream".into());
    Ok(HttpResponse::Ok().content_type(content_type).body(payload))
}
pub async fn batch(
    state: web::Data<AppState>,
    auth: Auth,
    body: web::Json<BatchRequest>,
) -> ApiResult {
    if body.ids.is_empty() || body.ids.len() > 100 {
        return Err(ApiError::bad("ids must contain 1 to 100 items"));
    }
    let ids = body
        .ids
        .iter()
        .map(|v| dto::parse_id(v))
        .collect::<Result<Vec<_>, _>>()?;
    let now = Utc::now();
    let mut update = message::Entity::update_many()
        .col_expr(message::Column::UpdatedAt, Expr::value(now))
        .filter(message::Column::Id.is_in(ids))
        .filter(Expr::col(message::Column::Id).in_subquery(repository::owned_ids(auth.user_id)));
    update = match body.action.as_str() {
        "mark_read" => update
            .col_expr(message::Column::Status, Expr::value("read"))
            .col_expr(message::Column::ReadAt, Expr::value(now)),
        "mark_unread" => update
            .col_expr(message::Column::Status, Expr::value("unread"))
            .col_expr(
                message::Column::ReadAt,
                Expr::value(Option::<chrono::DateTime<Utc>>::None),
            ),
        "pin" => update.col_expr(message::Column::Pinned, Expr::value(true)),
        "unpin" => update.col_expr(message::Column::Pinned, Expr::value(false)),
        "delete" => update.col_expr(message::Column::DeletedAt, Expr::value(now)),
        _ => return Err(ApiError::bad("unsupported batch action")),
    };
    let result = update.exec(&state.db).await?;
    Ok(success(json!({"affected": result.rows_affected})))
}

pub async fn stream(state: web::Data<AppState>, auth: Auth) -> ApiResult {
    let mut subscription = state.realtime.subscribe(auth.user_id);
    let stream = async_stream::stream! {
        yield Ok::<_, actix_web::Error>(web::Bytes::from(encode(&Event { id: None, kind: "ready".into(), data: json!({"connectedAt": Utc::now()}) })));
        let mut heartbeat = tokio::time::interval_at(tokio::time::Instant::now() + std::time::Duration::from_secs(20), std::time::Duration::from_secs(20));
        let expiry = tokio::time::sleep((auth.expires_at - Utc::now()).to_std().unwrap_or_default());
        tokio::pin!(expiry);
        loop {
            tokio::select! {
                _ = &mut expiry => {
                    yield Ok(web::Bytes::from(encode(&Event { id: None, kind: "auth.expired".into(), data: json!({}) })));
                    break;
                }
                _ = heartbeat.tick() => { yield Ok(web::Bytes::from_static(b": heartbeat\n\n")); }
                event = subscription.receiver.recv() => {
                    match event { Some(event) => yield Ok(web::Bytes::from(encode(&event))), None => break }
                }
            }
        }
        // Explicitly capture the whole guard, ensuring disconnect drops the subscription.
        drop(subscription);
    };
    Ok(HttpResponse::Ok()
        .insert_header(("Content-Type", "text/event-stream"))
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("X-Accel-Buffering", "no"))
        .streaming(stream))
}
