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
        _raw: &EventContent,
    ) -> MessageContent {
        let kind = &metadata.event_type;
        use super::JsonFieldExt;
        let preview = match &self.payload {
            StripeWebhookPayload::Snapshot(event) => {
                let resource = event.data.value().and_then(|v| v.object.value());
                use generated::StripeResource;
                match resource {
                    Some(StripeResource::PaymentIntent(v)) => {
                        summary(&[v.description.value_deref()])
                    }
                    Some(StripeResource::Charge(v)) => summary(&[v.description.value_deref()]),
                    Some(StripeResource::Invoice(v)) => summary(&[v.description.value_deref()]),
                    Some(StripeResource::Customer(v)) => {
                        summary(&[v.description.value_deref(), v.name.value_deref()])
                    }
                    _ => String::new(),
                }
            }
            StripeWebhookPayload::Thin(_) => String::new(),
        };
        // related_object.url may be an API-relative path, not a user-facing link.
        message(format!("Stripe {kind}"), preview, "normal", String::new())
    }
}
