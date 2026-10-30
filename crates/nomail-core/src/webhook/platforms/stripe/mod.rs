//! Stripe signatures, Snapshot/Thin event models and built-in inbox mapping.
#[cfg(test)]
mod tests;
use crate::webhook::{
    Authentication, EventMetadata, IncomingMessage, PlatformEvent, PlatformPayload, ReceiveOutcome,
    WebhookConfig, WebhookError, WebhookRequest, crypto, support::*,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StripeEvent {
    Snapshot(PlatformPayload),
    /// No API request is performed. Mapping provides a metadata-only summary.
    Thin(PlatformPayload),
}

pub(in crate::webhook) fn receive(
    config: &WebhookConfig<'_>,
    request: &WebhookRequest<'_>,
) -> Result<ReceiveOutcome, WebhookError> {
    match &config.authentication {
        Authentication::None => {}
        Authentication::StripeSigningSecret(secret) if !secret.is_empty() => {
            verify_signature(config, request, secret)?
        }
        _ => return Err(invalid_auth()),
    }
    let payload = object(request.body)?;
    let event_type = required(text(&payload, &["type"]), "missing Stripe event type")?;
    let id = required(text(&payload, &["id"]), "missing Stripe event id")?;
    let version = optional(text(&payload, &["api_version"]));
    let thin = match text(&payload, &["object"]).as_str() {
        "v2.core.event" => true,
        "event" => false,
        _ => {
            return Err(WebhookError::Unsupported(
                "unsupported Stripe event envelope",
            ));
        }
    };
    let data = data(payload, event_type, version, "stripe:event", id);
    Ok(ReceiveOutcome::Event(PlatformEvent::Stripe(if thin {
        StripeEvent::Thin(data)
    } else {
        StripeEvent::Snapshot(data)
    })))
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

impl StripeEvent {
    fn data(&self) -> &PlatformPayload {
        match self {
            Self::Snapshot(data) | Self::Thin(data) => data,
        }
    }

    pub fn metadata(&self) -> &EventMetadata {
        &self.data().metadata
    }

    pub fn to_message(&self) -> IncomingMessage {
        let data = self.data();
        let kind = &data.metadata.event_type;
        let level = if kind.ends_with(".failed") || kind.ends_with("_failed") {
            "critical"
        } else if kind.ends_with(".succeeded") {
            "success"
        } else {
            "normal"
        };
        let content = if matches!(self, Self::Thin(_)) {
            format!(
                "Stripe event {} (thin notification; details not fetched): {}",
                text(&data.payload, &["id"]),
                summary(data)
            )
        } else {
            summary(data)
        };
        // related_object.url may be an API-relative path, not a user-facing link.
        message(
            format!("Stripe {kind}"),
            content,
            "stripe".into(),
            level,
            String::new(),
        )
    }
}
