//! HTTP and live connection adaptation only; all message use cases are in Core.
use crate::{
    AppState,
    auth::Auth,
    error::{ApiError, ApiResult, success},
    realtime::{Event, encode},
};
use actix_web::{HttpResponse, web};
use chrono::Utc;
use nomail_core::{
    api::{
        endpoints as e,
        sse::{HEARTBEAT, HEARTBEAT_SECONDS},
    },
    message::{BatchRequest, ListQuery, UpdateRequest},
};

fn id(value: &str) -> Result<uuid::Uuid, ApiError> {
    uuid::Uuid::parse_str(value).map_err(|_| ApiError::bad("invalid message id"))
}
pub async fn list(
    state: web::Data<AppState>,
    auth: Auth,
    query: web::Query<ListQuery>,
) -> ApiResult {
    Ok(success::<e::ListMessages>(
        super::service(&state.db)
            .list(auth.user_id, query.into_inner())
            .await?,
    ))
}
pub async fn unread_count(state: web::Data<AppState>, auth: Auth) -> ApiResult {
    Ok(success::<e::UnreadCount>(
        super::service(&state.db).unread_count(auth.user_id).await?,
    ))
}
pub async fn get(state: web::Data<AppState>, auth: Auth, path: web::Path<String>) -> ApiResult {
    Ok(success::<e::GetMessage>(
        super::service(&state.db)
            .get(auth.user_id, id(&path)?)
            .await?,
    ))
}
pub async fn update(
    state: web::Data<AppState>,
    auth: Auth,
    path: web::Path<String>,
    body: web::Json<UpdateRequest>,
) -> ApiResult {
    Ok(success::<e::UpdateMessage>(
        super::service(&state.db)
            .update(auth.user_id, id(&path)?, body.into_inner(), Utc::now())
            .await?,
    ))
}
pub async fn delete(state: web::Data<AppState>, auth: Auth, path: web::Path<String>) -> ApiResult {
    super::service(&state.db)
        .delete(auth.user_id, id(&path)?, Utc::now())
        .await?;
    Ok(success::<e::DeleteMessage>(()))
}
pub async fn raw(state: web::Data<AppState>, auth: Auth, path: web::Path<String>) -> ApiResult {
    let raw = super::service(&state.db)
        .raw(auth.user_id, id(&path)?)
        .await?;
    Ok(HttpResponse::Ok()
        .content_type(raw.content_type)
        .body(raw.text))
}
pub async fn batch(
    state: web::Data<AppState>,
    auth: Auth,
    body: web::Json<BatchRequest>,
) -> ApiResult {
    Ok(success::<e::BatchMessages>(
        super::service(&state.db)
            .batch(auth.user_id, body.into_inner(), Utc::now())
            .await?,
    ))
}
pub async fn stream(state: web::Data<AppState>, auth: Auth) -> ApiResult {
    let mut subscription = state.realtime.subscribe(auth.user_id);
    let stream = async_stream::stream! {
        yield Ok::<_,actix_web::Error>(web::Bytes::from(encode(&Event::ready(Utc::now()))));
        let mut heartbeat=tokio::time::interval_at(tokio::time::Instant::now()+std::time::Duration::from_secs(HEARTBEAT_SECONDS),std::time::Duration::from_secs(HEARTBEAT_SECONDS));
        let expiry=tokio::time::sleep((auth.expires_at-Utc::now()).to_std().unwrap_or_default());tokio::pin!(expiry);
        loop {tokio::select! {
            _=&mut expiry=>{yield Ok(web::Bytes::from(encode(&Event::auth_expired())));break;}
            _=heartbeat.tick()=>{yield Ok(web::Bytes::from_static(HEARTBEAT));}
            event=subscription.receiver.recv()=>{match event{Some(event)=>yield Ok(web::Bytes::from(encode(&event))),None=>break}}
        }}
        drop(subscription);
    };
    Ok(HttpResponse::Ok()
        .insert_header(("Content-Type", "text/event-stream"))
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("X-Accel-Buffering", "no"))
        .streaming(stream))
}
