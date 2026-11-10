//! Telegram Bot API authentication, Update model and inbox mapping.
pub mod config;
mod models;
pub use models::*;
#[cfg(test)]
mod tests;
use crate::webhook::{
    Authentication, EventContent, EventMetadata, MessageContent, PlatformData, PlatformEvent,
    ReceiveOutcome, WebhookConfig, WebhookError, WebhookRequest, crypto, support::*,
};
use models::TelegramValue;

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
    let known: Vec<_> = fields
        .iter()
        .filter(|key| models::is_update_field(key))
        .collect();
    if known.len() > 1 || id < 0 {
        return Err(WebhookError::InvalidPayload("invalid Telegram Update"));
    }
    // Prefer a known branch over future metadata. Preserve a single unknown
    // branch, and accept update_id-only envelopes (the contract says at most one).
    let kind = known.first().map(|s| s.as_str()).unwrap_or_else(|| {
        if fields.len() == 1 {
            fields[0].as_str()
        } else {
            "update"
        }
    });
    let event = TelegramEvent::read(&payload)
        .ok_or(WebhookError::InvalidPayload("invalid Telegram Update"))?;
    Ok(PlatformEvent {
        metadata: metadata(kind.to_owned(), None, "telegram:bot-update", id.to_string()),
        data: PlatformData::Telegram(Box::new(event)),
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
        let update = payload.get(kind).unwrap_or(payload);
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
