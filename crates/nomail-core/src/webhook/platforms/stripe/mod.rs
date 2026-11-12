//! Stripe signatures, Snapshot/Thin event models and built-in inbox mapping.
pub mod config;
#[cfg(test)]
mod tests;
use super::native::NativeValue;
use crate::webhook::{
    Authentication, EventContent, EventMetadata, MessageContent, PlatformEvent, ReceiveOutcome,
    WebhookConfig, WebhookError, WebhookRequest, crypto, support::*,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

webhook_header!(StripeWebhookHeader {});
webhook!(StripeWebhook, StripeWebhookHeader, StripeWebhookPayload);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub enum StripeCreated {
    Unix(i64),
    Text(String),
}
impl NativeValue for StripeCreated {
    fn read(value: &Value) -> Option<Self> {
        value
            .as_i64()
            .map(Self::Unix)
            .or_else(|| value.as_str().map(|s| Self::Text(s.to_owned())))
    }
}
native_model!(StripeWebhookPayload {
    id: String = "id",
    object: String = "object",
    event_type: String = "type",
    api_version: String = "api_version",
    created: StripeCreated = "created",
    livemode: bool = "livemode",
    data: StripeEventData = "data",
    related_object: StripeRelatedObject = "related_object",
    account: String = "account",
    context: String = "context",
    pending_webhooks: i64 = "pending_webhooks",
    request: Value = "request",
});
native_model!(StripeEventData {
    // Resource shape depends on the event type and Stripe API version.
    object: Value = "object",
    previous_attributes: Value = "previous_attributes",
});
native_model!(StripeRelatedObject {
    id: String = "id",
    object_type: String = "type",
    url: String = "url",
});

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
    let payload = object(request.body)?;
    let event_type = required(text(&payload, &["type"]), "missing Stripe event type")?;
    let id = required(text(&payload, &["id"]), "missing Stripe event id")?;
    let version = optional(text(&payload, &["api_version"]));
    match text(&payload, &["object"]).as_str() {
        "v2.core.event" | "event" => {}
        _ => {
            return Err(WebhookError::Unsupported(
                "unsupported Stripe event envelope",
            ));
        }
    }
    Ok(PlatformEvent::from_payload(
        config.provider,
        metadata(event_type, version, "stripe:event", id),
        EventContent::Json(payload),
        request,
    )?
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
