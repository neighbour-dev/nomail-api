//! Shopify HMAC verification, versioned topic payloads and inbox mapping.
#[cfg(test)]
mod tests;
use crate::webhook::{
    Authentication, EventMetadata, IncomingMessage, PlatformEvent, PlatformPayload, ReceiveOutcome,
    WebhookConfig, WebhookError, WebhookRequest, crypto, support::*,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShopifyEvent {
    pub data: PlatformPayload,
    pub shop_domain: String,
    /// An operation identifier is distinct from the delivery deduplication key.
    pub operation_id: Option<String>,
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
    Ok(ReceiveOutcome::Event(PlatformEvent::Shopify(
        ShopifyEvent {
            data: data(
                payload,
                topic,
                version,
                &format!("shopify:{shop_domain}:delivery"),
                request.header("x-shopify-webhook-id").into(),
            ),
            shop_domain,
            operation_id: optional(request.header("x-shopify-event-id")),
        },
    )))
}

impl ShopifyEvent {
    pub fn metadata(&self) -> &EventMetadata {
        &self.data.metadata
    }

    pub fn to_message(&self) -> IncomingMessage {
        message(
            format!("Shopify {}", self.data.metadata.event_type),
            first(&[
                text(&self.data.payload, &["title"]),
                text(&self.data.payload, &["name"]),
                summary(&self.data),
            ]),
            self.shop_domain.clone(),
            "normal",
            String::new(),
        )
    }
}
