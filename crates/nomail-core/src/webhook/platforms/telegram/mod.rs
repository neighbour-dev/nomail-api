//! Telegram Bot API authentication, Update model and inbox mapping.
pub mod config;
mod models;
pub use models::*;
#[cfg(test)]
mod tests;
use crate::webhook::{
    Authentication, EventContent, EventMetadata, MessageContent, PlatformEvent, ReceiveOutcome,
    WebhookConfig, WebhookError, WebhookRequest, crypto, support::*,
};
use models::TelegramValue;

impl crate::webhook::platforms::native::NativeValue for TelegramWebhookPayload {
    fn read(value: &serde_json::Value) -> Option<Self> {
        <Self as TelegramValue>::read(value)
    }
}

// The Bot API secret-token header is authentication, not persisted event data.
webhook_header!(TelegramWebhookHeader {});
webhook!(
    TelegramWebhook,
    TelegramWebhookHeader,
    TelegramWebhookPayload
);

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
        .into_iter()
        .flat_map(|object| object.keys())
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
    let kind = if known.len() == 1 {
        known[0].as_str()
    } else if fields.len() == 1 {
        fields[0].as_str()
    } else {
        "update"
    };
    Ok(PlatformEvent::from_payload(
        config.provider,
        metadata(kind.to_owned(), None, "telegram:bot-update", id.to_string()),
        EventContent::Json(payload),
        request,
    )?
    .into())
}

impl TelegramWebhook {
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
