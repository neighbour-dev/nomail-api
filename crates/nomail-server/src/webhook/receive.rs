use super::parser;
use crate::{
    AppState, crypto,
    entity::webhook,
    error::{ApiError, ApiResult, success},
    message,
    realtime::Event,
};
use actix_web::{HttpRequest, HttpResponse, web};
use chrono::Utc;
use nomail_core::api::endpoints as e;
use nomail_core::webhook::ReceiveOutcome;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

pub async fn handle(
    state: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
) -> ApiResult {
    let config = webhook::Entity::find()
        .filter(webhook::Column::Key.eq(path.into_inner()))
        .filter(webhook::Column::Enabled.eq(true))
        .filter(webhook::Column::DeletedAt.is_null())
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::not_found("webhook config not found"))?;
    let provider = parser::provider(&config.provider)
        .map_err(|_| ApiError::internal("webhook provider is not configured"))?;
    let secret = match config.encrypted_secret.as_deref().filter(|s| !s.is_empty()) {
        Some(secret) => crypto::decrypt(secret, &state.config.encryption_key)?,
        None => String::new(),
    };
    let now = Utc::now();
    let event = match parser::receive(provider, req.headers(), &body, &secret, now.timestamp())? {
        ReceiveOutcome::Event(event) => event,
        ReceiveOutcome::Respond(response) => {
            let status = actix_web::http::StatusCode::from_u16(response.status)
                .map_err(ApiError::internal)?;
            return Ok(HttpResponse::build(status)
                .content_type(response.content_type)
                .body(response.body));
        }
        ReceiveOutcome::Ignored { .. } => {
            return Ok(success::<e::ReceiveWebhook>(
                nomail_core::webhook::ReceiveResponse::Ignored { ignored: true },
            ));
        }
    };
    let accepted = message::service(&state.db)
        .accept(
            config.id,
            *event,
            config.source.as_deref(),
            Uuid::now_v7(),
            now,
        )
        .await?;
    if let Some(event) = accepted.created_event {
        state
            .realtime
            .publish(config.user_id, Event::MessageCreated(event));
    }
    Ok(success::<e::ReceiveWebhook>(
        nomail_core::webhook::ReceiveResponse::Message {
            message_id: accepted.outcome.id(),
            duplicate: accepted.outcome.duplicate(),
        },
    ))
}
