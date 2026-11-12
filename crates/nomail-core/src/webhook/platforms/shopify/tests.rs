use crate::webhook::{test_support::*, *};
use base64::{Engine, engine::general_purpose::STANDARD};

#[test]
fn cropped_fields_versions_and_distinct_identifiers() {
    let body = br#"{"id":123,"variants":[{"price":"29.99"}]}"#;
    let h = headers(&[
        (
            "X-Shopify-Hmac-Sha256",
            &STANDARD.encode(sign(b"client-secret", body)),
        ),
        ("X-Shopify-Topic", "products/update"),
        ("X-Shopify-Shop-Domain", "demo.myshopify.com"),
        ("X-Shopify-API-Version", "2026-10"),
        ("X-Shopify-Webhook-Id", "delivery-1"),
        ("X-Shopify-Event-Id", "operation-1"),
    ]);
    let parsed = event(
        receive_with(
            Provider::Shopify,
            Authentication::ShopifyClientSecret("client-secret"),
            &h,
            body,
        )
        .unwrap(),
    );
    assert_eq!(parsed.metadata().version.as_deref(), Some("2026-10"));
    assert_eq!(
        parsed.metadata().deduplication_key.as_ref().unwrap().value,
        "delivery-1"
    );
    assert_eq!(
        parsed.to_message_content().source.as_deref(),
        Some("demo.myshopify.com")
    );
    assert!(parsed.to_message_content().content.contains("29.99"));
    let Some(PlatformData::Shopify(event)) = parsed.data else {
        panic!()
    };
    assert_eq!(event.header.event_id.as_deref(), Some("operation-1"));
    assert_eq!(event.payload.id.into_value(), Some(serde_json::json!(123)));
    assert_eq!(
        receive_with(
            Provider::Shopify,
            Authentication::ShopifyClientSecret("wrong"),
            &h,
            body
        )
        .unwrap_err(),
        WebhookError::Unauthorized
    );
}
