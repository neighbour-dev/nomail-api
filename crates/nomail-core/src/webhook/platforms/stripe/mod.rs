//! Stripe signatures, Snapshot/Thin event models and built-in inbox mapping.
pub mod config;
#[macro_use]
mod support;
mod generated;
mod models;
pub use generated::*;
pub use models::*;
pub use support::StripeNumber;
pub(in crate::webhook) const RAW_HEADER_BLACKLIST: &[&str] = &["stripe-signature"];
#[cfg(test)]
mod tests;
use crate::webhook::{
    Authentication, EventContent, EventMetadata, MessageContent, PlatformEvent, ReceiveOutcome,
    WebhookConfig, WebhookError, WebhookRequest, crypto, support::*,
};
use serde_json::Value;

webhook_header!(StripeWebhookHeader {});
webhook!(StripeWebhook, StripeWebhookHeader, StripeWebhookPayload);

pub(in crate::webhook) fn receive(
    config: &WebhookConfig<'_>,
    request: &WebhookRequest<'_>,
) -> Result<ReceiveOutcome, WebhookError> {
    match &config.authentication {
        Authentication::None => {}
        Authentication::StripeSigningSecret(secret) if !secret.is_empty() => {
            verify_signature(config, request, secret)?
        }
        Authentication::StripeSigningSecrets(secrets)
            if !secrets.is_empty() && secrets.iter().all(|s| !s.is_empty()) =>
        {
            if !secrets
                .iter()
                .any(|secret| verify_signature(config, request, secret).is_ok())
            {
                return Err(WebhookError::Unauthorized);
            }
        }
        _ => return Err(invalid_auth()),
    }
    let payload = json_payload(request.body)?;
    let event_type = text(&payload, &["type"]);
    let id = payload
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let version = optional(text(&payload, &["api_version"]));
    Ok(PlatformEvent::from_raw(
        config.provider,
        metadata(event_type, version, "stripe:event", id),
        EventContent::Json(payload),
        request,
    )
    .into())
}

fn verify_signature(
    config: &WebhookConfig<'_>,
    request: &WebhookRequest<'_>,
    secret: &str,
) -> Result<(), WebhookError> {
    let mut times = Vec::new();
    let mut signatures = Vec::new();
    for value in request.headers.values("stripe-signature") {
        for part in value.split(',') {
            match part.trim().split_once('=') {
                Some(("t", value)) => times.push(value),
                Some(("v1", value)) => signatures.push(value),
                _ => {}
            }
        }
    }
    if times.len() != 1 {
        return Err(WebhookError::Unauthorized);
    }
    crypto::timestamp(config, times[0])?;
    for signature in signatures {
        if let Ok(signature) = crypto::hex_signature(signature)
            && crypto::hmac(
                secret.as_bytes(),
                &[times[0].as_bytes(), b".", request.body],
                &signature,
            )
            .is_ok()
        {
            return Ok(());
        }
    }
    Err(WebhookError::Unauthorized)
}

impl StripeWebhook {
    pub fn to_message_content(
        &self,
        metadata: &EventMetadata,
        raw: &EventContent,
    ) -> MessageContent {
        let kind = &metadata.event_type;
        let content = if raw
            .as_json()
            .is_some_and(|p| text(p, &["object"]) == "v2.core.event")
        {
            format!(
                "Stripe event {} (thin notification; details not fetched): {}",
                raw.as_json()
                    .map(|value| text(value, &["id"]))
                    .unwrap_or_default(),
                raw
            )
        } else {
            raw.to_string()
        };
        // related_object.url may be an API-relative path, not a user-facing link.
        message(
            format!("Stripe {kind}"),
            content,
            "stripe".into(),
            "normal",
            String::new(),
        )
    }
}
