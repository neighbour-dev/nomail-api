//! Telegram Bot API authentication, Update model and inbox mapping.
pub mod config;
pub(in crate::webhook) const RAW_HEADER_BLACKLIST: &[&str] = &["x-telegram-bot-api-secret-token"];
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
    let payload = json_payload(request.body)?;
    let id = payload
        .get("update_id")
        .and_then(|v| v.as_i64().filter(|id| *id >= 0))
        .map(|id| id.to_string())
        .unwrap_or_default();
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
    // Prefer a known branch over future metadata. Preserve a single unknown
    // branch, and accept update_id-only envelopes (the contract says at most one).
    let kind = if known.len() == 1 {
        known[0].as_str()
    } else if fields.len() == 1 {
        fields[0].as_str()
    } else {
        "update"
    };
    Ok(PlatformEvent::from_raw(
        config.provider,
        metadata(kind.to_owned(), None, "telegram:bot-update", id),
        EventContent::Json(payload),
        request,
    )
    .into())
}

impl TelegramWebhook {
    pub fn to_message_content(
        &self,
        metadata: &EventMetadata,
        _raw: &EventContent,
    ) -> MessageContent {
        let kind = &metadata.event_type;
        use super::JsonFieldExt;
        let p = &self.payload;
        let msg = match kind.as_str() {
            "message" => p.message.value(),
            "edited_message" => p.edited_message.value(),
            "channel_post" => p.channel_post.value(),
            "edited_channel_post" => p.edited_channel_post.value(),
            "business_message" => p.business_message.value(),
            "edited_business_message" => p.edited_business_message.value(),
            "guest_message" => p.guest_message.value(),
            _ => None,
        };
        let preview = match kind.as_str() {
            "callback_query" => {
                summary(&[p.callback_query.value().and_then(|v| v.data.value_deref())])
            }
            "inline_query" => {
                summary(&[p.inline_query.value().and_then(|v| v.query.value_deref())])
            }
            "chosen_inline_result" => summary(&[p
                .chosen_inline_result
                .value()
                .and_then(|v| v.query.value_deref())]),
            "poll" => summary(&[p.poll.value().and_then(|v| v.question.value_deref())]),
            _ => summary(&[
                msg.and_then(|v| v.text.value_deref()),
                msg.and_then(|v| v.caption.value_deref()),
            ]),
        };
        message(format!("Telegram {kind}"), preview, "normal", String::new())
    }
}
