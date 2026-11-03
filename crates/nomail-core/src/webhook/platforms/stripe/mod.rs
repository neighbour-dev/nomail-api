//! Stripe signatures, Snapshot/Thin event models and built-in inbox mapping.
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
pub enum StripeEvent {
    Snapshot(StripeSnapshot),
    /// No API request is performed. Mapping provides a metadata-only summary.
    Thin(StripeThinNotification),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct StripeSnapshot {
    pub object_id: Option<String>,
    pub object_type: Option<String>,
    pub created: Option<i64>,
    pub livemode: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct StripeThinNotification {
    pub created: Option<String>,
    pub related_object: Option<StripeRelatedObject>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct StripeRelatedObject {
    pub id: Option<String>,
    pub object_type: Option<String>,
    /// API resource path, not a user-facing URL or an instruction to fetch it.
    pub url: Option<String>,
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
    let thin = match text(&payload, &["object"]).as_str() {
        "v2.core.event" => true,
        "event" => false,
        _ => {
            return Err(WebhookError::Unsupported(
                "unsupported Stripe event envelope",
            ));
        }
    };
    let event = if thin {
        StripeEvent::Thin(StripeThinNotification {
            created: payload["created"].as_str().map(str::to_owned),
            related_object: payload
                .get("related_object")
                .filter(|value| value.is_object())
                .map(|value| StripeRelatedObject {
                    id: optional(text(value, &["id"])),
                    object_type: optional(text(value, &["type"])),
                    url: optional(text(value, &["url"])),
                }),
        })
    } else {
        StripeEvent::Snapshot(StripeSnapshot {
            object_id: optional(text(&payload, &["data", "object", "id"])),
            object_type: optional(text(&payload, &["data", "object", "object"])),
            created: payload["created"].as_i64(),
            livemode: payload["livemode"].as_bool(),
        })
    };
    Ok(PlatformEvent {
        metadata: metadata(event_type, version, "stripe:event", id),
        data: PlatformData::Stripe(event),
        raw: EventContent::Json(payload),
    }
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

impl StripeEvent {
    pub fn to_message_content(
        &self,
        metadata: &EventMetadata,
        raw: &EventContent,
    ) -> MessageContent {
        let kind = &metadata.event_type;
        let content = if matches!(self, Self::Thin(_)) {
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
