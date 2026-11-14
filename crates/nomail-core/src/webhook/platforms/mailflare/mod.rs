//! Independently modeled Mailflare notification protocol; no upstream code is copied.
//! The signature covers the body only, not delivery/event/attempt headers.
pub mod config;
pub(in crate::webhook) const RAW_HEADER_BLACKLIST: &[&str] = &["x-email-platform-signature"];
#[cfg(test)]
mod tests;

use crate::webhook::{
    Authentication, EventContent, EventMetadata, JsonFieldExt, MessageContent, PlatformEvent,
    ReceiveOutcome, WebhookConfig, WebhookError, WebhookRequest, crypto, support::*,
};

webhook_header!(MailflareWebhookHeader {
    event = "x-email-platform-event",
    delivery = "x-email-platform-delivery",
    attempt = "x-email-platform-attempt",
});
webhook!(
    MailflareWebhook,
    MailflareWebhookHeader,
    MailflareWebhookPayload
);

native_model!(MailflareWebhookPayload {
    event_type: String = "type",
    test: bool = "test",
    data: MailflareMessage = "data",
});

native_model!(MailflareMessage {
    message_id: String = "messageId",
    provider_message_id: String = "providerMessageId",
    from: String = "from",
    to: String = "to",
    cc: String = "cc",
    subject: String = "subject",
    thread_id: String = "threadId",
    spam_score: serde_json::Number = "spamScore",
    spam_verdict: String = "spamVerdict",
    error: String = "error",
});

pub(in crate::webhook) fn receive(
    config: &WebhookConfig<'_>,
    request: &WebhookRequest<'_>,
) -> Result<ReceiveOutcome, WebhookError> {
    let Authentication::HmacSha256(secret) = &config.authentication else {
        return Err(invalid_auth());
    };
    if secret.is_empty() {
        return Err(invalid_auth());
    }
    let signatures = request.headers.values("x-email-platform-signature");
    if signatures.len() != 1 || signatures[0].len() != 64 {
        return Err(WebhookError::Unauthorized);
    }
    let signature = crypto::hex_signature(signatures[0])?;
    crypto::hmac(secret.as_bytes(), &[request.body], &signature)?;
    let payload = json_payload(request.body)?;
    // Only the signed body can identify the event; headers are descriptive.
    let event_type = text(&payload, &["type"]);
    Ok(PlatformEvent::from_raw(
        config.provider,
        metadata(
            event_type,
            None,
            "mailflare:delivery",
            request.header("x-email-platform-delivery").into(),
        ),
        EventContent::Json(payload),
        request,
    )
    .into())
}

impl MailflareWebhook {
    pub fn to_message_content(
        &self,
        metadata: &EventMetadata,
        _raw: &EventContent,
    ) -> MessageContent {
        let data = self.payload.data.value();
        let subject = data
            .and_then(|data| data.subject.value_deref())
            .unwrap_or("")
            .trim();
        let title = if subject.is_empty() {
            format!("Mailflare {}", metadata.event_type)
        } else {
            subject.to_owned()
        };
        message(
            title,
            summary(&[
                data.and_then(|v| v.error.value_deref()),
                data.and_then(|v| v.subject.value_deref()),
                data.and_then(|v| v.from.value_deref()),
                data.and_then(|v| v.to.value_deref()),
            ]),
            if metadata.event_type == "message.failed" {
                "high"
            } else {
                "normal"
            },
            String::new(),
        )
    }
}
