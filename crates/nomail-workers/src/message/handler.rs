//! Workers HTTP adaptation only. No SQL or message lifecycle rules.
use crate::{
    db::Store,
    error::{ApiError, Result, success},
    time,
};
use chrono::{DateTime, Utc};
use nomail_core::{
    api::endpoints as e,
    message::{BatchRequest, ListQuery, UpdateRequest},
};
use worker::Response;
fn now() -> DateTime<Utc> {
    DateTime::from_timestamp_millis(time::now_ms()).expect("runtime timestamp in range")
}
fn id(value: &str) -> Result<uuid::Uuid> {
    uuid::Uuid::parse_str(value).map_err(|_| ApiError::bad("invalid message id"))
}
pub async fn list(query: ListQuery, user_id: i64, db: &Store) -> Result<Response> {
    success::<e::ListMessages>(super::service(db).list(user_id, query).await?)
}
pub async fn unread_count(user_id: i64, db: &Store) -> Result<Response> {
    success::<e::UnreadCount>(super::service(db).unread_count(user_id).await?)
}
pub async fn get(value: &str, user_id: i64, db: &Store) -> Result<Response> {
    success::<e::GetMessage>(super::service(db).get(user_id, id(value)?).await?)
}
pub async fn update(
    value: &str,
    request: UpdateRequest,
    user_id: i64,
    db: &Store,
) -> Result<Response> {
    success::<e::UpdateMessage>(
        super::service(db)
            .update(user_id, id(value)?, request, now())
            .await?,
    )
}
pub async fn delete(value: &str, user_id: i64, db: &Store) -> Result<Response> {
    super::service(db)
        .delete(user_id, id(value)?, now())
        .await?;
    success::<e::DeleteMessage>(())
}
pub async fn batch(request: BatchRequest, user_id: i64, db: &Store) -> Result<Response> {
    success::<e::BatchMessages>(super::service(db).batch(user_id, request, now()).await?)
}
pub async fn raw(value: &str, user_id: i64, db: &Store) -> Result<Response> {
    let raw = super::service(db).raw(user_id, id(value)?).await?;
    let mut response = Response::ok(raw.text)?;
    response
        .headers_mut()
        .set("Content-Type", &raw.content_type)?;
    Ok(response)
}
