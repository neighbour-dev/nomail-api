//! Shopify HMAC verification, versioned topic payloads and inbox mapping.
#[cfg(test)]
mod tests;
use crate::webhook::{
    Authentication, EventContent, EventMetadata, IncomingMessage, PlatformData, PlatformEvent,
    ReceiveOutcome, WebhookConfig, WebhookError, WebhookRequest, crypto, support::*,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShopifyEvent {
    pub shop_domain: String,
    /// An operation identifier is distinct from the delivery deduplication key.
    pub operation_id: Option<String>,
    pub resource_id: Option<String>,
}

pub(in crate::webhook) fn receive(
    config: &WebhookConfig<'_>,
    request: &WebhookRequest<'_>,
) -> Result<ReceiveOutcome, WebhookError> {
    match &config.authentication {
        Authentication::None => {}
        Authentication::ShopifyClientSecret(secret) if !secret.is_empty() => {
            let signature = STANDARD
                .decode(request.header("x-shopify-hmac-sha256"))
                .map_err(|_| WebhookError::Unauthorized)?;
            crypto::hmac(secret.as_bytes(), &[request.body], &signature)?;
        }
        _ => return Err(invalid_auth()),
    }
    let payload = object(request.body)?;
    let topic = required(
        request.header("x-shopify-topic").into(),
        "missing Shopify topic",
    )?;
    let shop_domain = required(
        request.header("x-shopify-shop-domain").into(),
        "missing Shopify shop domain",
    )?;
    let version = optional(request.header("x-shopify-api-version"));
    let event = ShopifyEvent {
        resource_id: optional(text(&payload, &["id"])),
        shop_domain: shop_domain.clone(),
        operation_id: optional(request.header("x-shopify-event-id")),
    };
    Ok(PlatformEvent {
        metadata: metadata(
            topic,
            version,
            &format!("shopify:{shop_domain}:delivery"),
            request.header("x-shopify-webhook-id").into(),
        ),
        data: PlatformData::Shopify(event),
        raw: EventContent::Json(payload),
    }
    .into())
}

impl ShopifyEvent {
    pub fn to_message(&self, metadata: &EventMetadata, raw: &EventContent) -> IncomingMessage {
        let payload = raw.as_json().unwrap_or(&serde_json::Value::Null);
        message(
            format!("Shopify {}", metadata.event_type),
            first(&[
                text(payload, &["title"]),
                text(payload, &["name"]),
                raw.to_string(),
            ]),
            self.shop_domain.clone(),
            "normal",
            String::new(),
        )
    }
}
