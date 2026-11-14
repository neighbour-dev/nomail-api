//! Shopify HMAC verification, versioned topic payloads and inbox mapping.
pub mod config;
pub(in crate::webhook) const RAW_HEADER_BLACKLIST: &[&str] = &["x-shopify-hmac-sha256"];
#[cfg(test)]
mod tests;
use crate::webhook::{
    Authentication, EventContent, EventMetadata, MessageContent, PlatformEvent, ReceiveOutcome,
    WebhookConfig, WebhookError, WebhookRequest, crypto, support::*,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::Value;

webhook_header!(ShopifyWebhookHeader {
    topic = "x-shopify-topic", shop_domain = "x-shopify-shop-domain", api_version = "x-shopify-api-version",
    event_id = "x-shopify-event-id", webhook_id = "x-shopify-webhook-id", triggered_at = "x-shopify-triggered-at",
});
// Shopify permits field-cropped, versioned resources. IDs retain their native
// JSON number/string representation instead of becoming resource_id strings.
native_model!(ShopifyWebhookPayload {
    id: Value = "id",
    admin_graphql_api_id: String = "admin_graphql_api_id",
    title: String = "title",
    name: String = "name",
});
webhook!(ShopifyWebhook, ShopifyWebhookHeader, ShopifyWebhookPayload);

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
    let payload = json_payload(request.body)?;
    let topic = request.header("x-shopify-topic").trim().to_owned();
    let shop_domain = request.header("x-shopify-shop-domain").trim().to_owned();
    let version = optional(request.header("x-shopify-api-version"));
    Ok(PlatformEvent::from_raw(
        config.provider,
        metadata(
            topic,
            version,
            &format!("shopify:{shop_domain}:delivery"),
            request.header("x-shopify-webhook-id").into(),
        ),
        EventContent::Json(payload),
        request,
    )
    .into())
}

impl ShopifyWebhook {
    pub fn to_message_content(
        &self,
        metadata: &EventMetadata,
        _raw: &EventContent,
    ) -> MessageContent {
        use super::JsonFieldExt;
        message(
            format!("Shopify {}", metadata.event_type),
            summary(&[
                self.payload.title.value_deref(),
                self.payload.name.value_deref(),
            ]),
            "normal",
            String::new(),
        )
    }
}
