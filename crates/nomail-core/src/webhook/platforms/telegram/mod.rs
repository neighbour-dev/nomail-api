//! Telegram Bot API authentication, Update model and inbox mapping.
pub mod config;
#[cfg(test)]
mod tests;
use crate::webhook::{
    Authentication, EventContent, EventMetadata, MessageContent, PlatformData, PlatformEvent,
    ReceiveOutcome, WebhookConfig, WebhookError, WebhookRequest, crypto, support::*,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct TelegramEvent {
    pub update_id: i64,
    pub chat_id: Option<i64>,
    pub message_id: Option<i64>,
    pub callback_query_id: Option<String>,
}

pub(in crate::webhook) fn receive(
    config: &WebhookConfig<'_>,
    request: &WebhookRequest<'_>,
) -> Result<ReceiveOutcome, WebhookError> {
    match &config.authentication {
        Authentication::None => {}
        Authentication::TelegramSecretToken(secret) if !secret.is_empty() => crypto::equal(
            secret.as_bytes(),
            request.header("x-telegram-bot-api-secret-token").as_bytes(),
        )?,
        _ => return Err(invalid_auth()),
    }
    let payload = object(request.body)?;
    let id = payload["update_id"]
        .as_i64()
        .ok_or(WebhookError::InvalidPayload("missing Telegram update_id"))?;
    let fields: Vec<_> = payload
        .as_object()
        .expect("object was checked")
        .keys()
        .filter(|key| key.as_str() != "update_id")
        .cloned()
        .collect();
    // Preserve unknown single update fields when Telegram adds new types.
    if fields.len() != 1 || id < 0 {
        return Err(WebhookError::InvalidPayload("invalid Telegram Update"));
    }
    let update = &payload[&fields[0]];
    let callback = fields[0] == "callback_query";
    let message = if callback { &update["message"] } else { update };
    let event = TelegramEvent {
        update_id: id,
        chat_id: message["chat"]["id"].as_i64(),
        message_id: message["message_id"].as_i64(),
        callback_query_id: if callback {
            optional(text(update, &["id"]))
        } else {
            None
        },
    };
    Ok(PlatformEvent {
        metadata: metadata(
            fields[0].clone(),
            None,
            "telegram:bot-update",
            id.to_string(),
        ),
        data: PlatformData::Telegram(event),
        raw: EventContent::Json(payload),
    }
    .into())
}

impl TelegramEvent {
    pub fn to_message_content(
        &self,
        metadata: &EventMetadata,
        raw: &EventContent,
    ) -> MessageContent {
        let kind = &metadata.event_type;
        let payload = raw.as_json().unwrap_or(&serde_json::Value::Null);
        let update = &payload[kind];
        message(
            format!("Telegram {kind}"),
            first(&[
                text(update, &["text"]),
                text(update, &["caption"]),
                text(update, &["data"]),
                update.to_string(),
            ]),
            first(&[
                text(update, &["chat", "title"]),
                text(update, &["chat", "username"]),
                "telegram".into(),
            ]),
            "normal",
            String::new(),
        )
    }
}
