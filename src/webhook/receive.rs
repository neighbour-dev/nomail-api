use super::parser;
use crate::{
    AppState, crypto,
    entity::{message, webhook},
    error::{ApiError, ApiResult, success},
    realtime::Event,
};
use actix_web::{HttpRequest, HttpResponse, web};
use chrono::{Duration, Utc};
use nomail_core::webhook::ReceiveOutcome;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set, sea_query::OnConflict};
use serde_json::json;
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
    let secret = match config.encrypted_secret.as_deref().filter(|v| !v.is_empty()) {
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
        ReceiveOutcome::Ignored { .. } => return Ok(success(json!({"ignored": true}))),
    };
    let parsed = event.to_message();
    let level = parsed.level.as_str();
    let expires = now + Duration::days(parser::retention_days(level));
    let external: String = event
        .metadata()
        .deduplication_key
        .as_ref()
        .map(|key| key.value.as_str())
        .unwrap_or("")
        .trim()
        .chars()
        .take(255)
        .collect();
    let external = if external.is_empty() {
        None
    } else {
        Some(external)
    };
    let id = Uuid::now_v7();
    let title: String = parsed.title.trim().chars().take(255).collect();
    let title = if title.is_empty() {
        "Webhook notification".into()
    } else {
        title
    };
    let source = if parsed.source.as_deref().unwrap_or("").trim().is_empty() {
        config
            .source
            .as_deref()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or(provider)
            .trim()
    } else {
        parsed.source.as_deref().unwrap_or("").trim()
    };
    let source: String = source.chars().take(100).collect();
    let content_type: String = req
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .trim()
        .chars()
        .take(100)
        .collect();
    let active = message::ActiveModel {
        id: Set(id),
        created_at: Set(now),
        updated_at: Set(now),
        deleted_at: Set(None),
        webhook_config_id: Set(config.id),
        external_id: Set(external.clone()),
        source: Set(Some(source.clone())),
        title: Set(title.clone()),
        content: Set(Some(parsed.content.trim().into())),
        level: Set(level.into()),
        status: Set("unread".into()),
        url: Set(Some(parsed.url.as_deref().unwrap_or("").trim().into())),
        raw_payload: Set(Some(String::from_utf8_lossy(&body).into_owned())),
        content_type: Set(Some(content_type)),
        pinned: Set(Some(false)),
        expire_at: Set(Some(expires)),
        read_at: Set(None),
    };
    let affected = message::Entity::insert(active)
        .on_conflict(
            OnConflict::columns([
                message::Column::WebhookConfigId,
                message::Column::ExternalId,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(&state.db)
        .await?;
    let duplicate = affected == 0;
    let id = if duplicate {
        // Intentionally unscoped: a deleted message still owns its idempotency key.
        message::Entity::find()
            .filter(message::Column::WebhookConfigId.eq(config.id))
            .filter(message::Column::ExternalId.eq(external))
            .one(&state.db)
            .await?
            .ok_or_else(|| ApiError::internal("duplicate message disappeared"))?
            .id
    } else {
        state.realtime.publish(config.user_id, Event { id: Some(id.to_string()), kind: "message.created".into(),
            data: json!({"id": id, "title": title, "source": source, "level": level, "status": "unread",
                "pinned": false, "webhookConfigId": config.id, "createdAt": now, "expireAt": expires}) });
        id
    };
    Ok(success(json!({"messageId": id, "duplicate": duplicate})))
}
