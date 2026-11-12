use crate::webhook::{test_support::*, *};
use serde_json::{Value, json};
use std::collections::BTreeSet;

// Shapes and attribute values from Apple's official webhook-events article,
// retrieved 2026-11-10. IDs and URLs are replaced with synthetic placeholders.
// The fixture is checked in, so CI needs no Apple account or network access.
// https://developer.apple.com/documentation/appstoreconnectapi/webhook-events
fn examples() -> Vec<Value> {
    serde_json::from_str(include_str!("fixtures/documented_events.json")).unwrap()
}

fn signature(secret: &str, body: &[u8]) -> String {
    format!("hmacsha256={}", hex::encode(sign(secret.as_bytes(), body)))
}

fn parse(payload: &Value) -> PlatformEvent {
    let body = serde_json::to_vec(payload).unwrap();
    let h = headers(&[
        ("Content-Type", "application/json"),
        ("X-Apple-Signature", &signature("test-secret", &body)),
    ]);
    let parsed = event(
        receive_with(
            Provider::AppStoreConnect,
            Authentication::HmacSha256("test-secret"),
            &h,
            &body,
        )
        .unwrap(),
    );
    assert_eq!(parsed.raw.as_json(), Some(payload));
    let decoded: PlatformEvent =
        serde_json::from_value(serde_json::to_value(&parsed).unwrap()).unwrap();
    assert_eq!(decoded, parsed);
    parsed
}

fn model(parsed: &PlatformEvent) -> &super::AppStoreConnectWebhookPayload {
    let PlatformData::AppStoreConnect(value) = &parsed.data else {
        panic!("wrong platform model")
    };
    &value.payload
}

// Every field in the documented sample must appear in the typed projection
// with its original type/value. Do not compute expected values with the same
// forgiving extractors as the implementation (which can conceal dropped data).
fn assert_documented_fields(projected: &Value, original: &Value, path: &str) {
    match original {
        Value::Object(fields) => {
            let object = projected
                .as_object()
                .unwrap_or_else(|| panic!("{path}: object missing"));
            for (key, value) in fields {
                assert_documented_fields(
                    object
                        .get(key)
                        .unwrap_or_else(|| panic!("{path}.{key}: field missing")),
                    value,
                    &format!("{path}.{key}"),
                );
            }
        }
        Value::Array(values) => {
            let actual = projected
                .as_array()
                .unwrap_or_else(|| panic!("{path}: array missing"));
            assert_eq!(actual.len(), values.len(), "{path}");
            for (i, (actual, expected)) in actual.iter().zip(values).enumerate() {
                assert_documented_fields(actual, expected, &format!("{path}[{i}]"));
            }
        }
        _ => assert_eq!(projected, original, "{path}: type/value mismatch"),
    }
}

#[test]
fn all_official_examples_retain_every_documented_field_in_typed_models() {
    let examples = examples();
    assert_eq!(examples.len(), 14); // 12 event types, two extra state-transition examples.
    let mut types = BTreeSet::new();
    for payload in &examples {
        let parsed = parse(payload);
        let projection = model(&parsed);
        assert_documented_fields(&serde_json::to_value(projection).unwrap(), payload, "event");
        assert_eq!(
            &serde_json::to_value(projection).unwrap(),
            payload,
            "invented fields in documented payload"
        );
        let kind = payload["data"]["type"].as_str().unwrap();
        types.insert(kind);
        assert_eq!(parsed.provider(), Provider::AppStoreConnect);
        assert_eq!(parsed.metadata.event_type, kind);
        assert_eq!(projection.data.event_type, kind);
        let key = parsed.metadata.deduplication_key.as_ref().unwrap();
        assert_eq!(key.scope, "appstoreconnect:event");
        assert_eq!(key.value, payload["data"]["id"].as_str().unwrap());
        assert_eq!(
            projection.data.version.into_value(),
            payload["data"].get("version").and_then(Value::as_u64)
        );
        assert_eq!(
            parsed.metadata.version,
            payload["data"].get("version").map(|v| v.to_string())
        );
        let summary = parsed.to_message_content();
        assert_eq!(summary.source.as_deref(), Some("App Store Connect"));
        assert_eq!(summary.level, MessageLevel::Normal);
        assert!(
            !summary.content.starts_with('{'),
            "known event fell back to raw: {kind}"
        );
    }
    let expected = BTreeSet::from([
        "appStoreVersionAppVersionStateUpdated",
        "buildBetaDetailExternalBuildStateUpdated",
        "buildUploadStateUpdated",
        "betaFeedbackScreenshotSubmissionCreated",
        "betaFeedbackCrashSubmissionCreated",
        "backgroundAssetVersionStateUpdated",
        "backgroundAssetVersionInternalBetaReleaseCreated",
        "backgroundAssetVersionExternalBetaReleaseStateUpdated",
        "backgroundAssetVersionAppStoreReleaseStateUpdated",
        "alternativeDistributionPackageVersionCreated",
        "alternativeDistributionPackageAvailableUpdated",
        "alternativeDistributionTerritoryAvailabilityUpdated",
    ]);
    assert_eq!(types, expected);
}

#[test]
fn official_signature_vector_and_exact_prefix_are_verified() {
    // Apple's published vector; this is not a JSON business notification.
    let body = b"Hello, World!";
    let expected = "hmacsha256=7f062172b01cb00b53ca068614674a3d982a34062a0f5d37687d5e3377e54657";
    assert_eq!(signature("This is my secret", body), expected);
    let h = headers(&[("X-Apple-Signature", expected)]);
    assert!(
        super::verify_signature("This is my secret", &WebhookRequest { headers: &h, body }).is_ok()
    );
    for malformed in [
        "",
        "sha256=00",
        "hmacsha256=00",
        "hmacsha256=not-hex",
        expected.strip_prefix("hmacsha256=").unwrap(),
    ] {
        let bad = headers(&[("x-apple-signature", malformed)]);
        assert_eq!(
            super::verify_signature(
                "This is my secret",
                &WebhookRequest {
                    headers: &bad,
                    body
                }
            ),
            Err(WebhookError::Unauthorized)
        );
    }
    let wrong_prefix = expected.replacen("hmacsha256=", "sha256=", 1);
    let bad = headers(&[("x-apple-signature", &wrong_prefix)]);
    assert_eq!(
        super::verify_signature(
            "This is my secret",
            &WebhookRequest {
                headers: &bad,
                body
            }
        ),
        Err(WebhookError::Unauthorized)
    );
}

#[test]
fn authentication_covers_original_body_and_rejects_duplicates_and_fallback_headers() {
    let body = "{\n \"data\": {\"type\":\"futureEvent\",\"id\":\"event-1\",\"attributes\":{\"future\":\"中文 🚀\"}}\n}".as_bytes();
    let h = headers(&[("x-apple-signature", &signature("test-secret", body))]);
    assert!(
        receive_with(
            Provider::AppStoreConnect,
            Authentication::HmacSha256("test-secret"),
            &h,
            body
        )
        .is_ok()
    );
    let reserialized = serde_json::to_vec(&serde_json::from_slice::<Value>(body).unwrap()).unwrap();
    assert_eq!(
        receive_with(
            Provider::AppStoreConnect,
            Authentication::HmacSha256("test-secret"),
            &h,
            &reserialized
        )
        .unwrap_err(),
        WebhookError::Unauthorized
    );
    assert_eq!(
        receive_with(
            Provider::AppStoreConnect,
            Authentication::HmacSha256("wrong"),
            &h,
            body
        )
        .unwrap_err(),
        WebhookError::Unauthorized
    );
    let mut duplicate = h.clone();
    duplicate.push(("X-Apple-Signature".into(), signature("test-secret", body)));
    assert_eq!(
        receive_with(
            Provider::AppStoreConnect,
            Authentication::HmacSha256("test-secret"),
            &duplicate,
            body
        )
        .unwrap_err(),
        WebhookError::Unauthorized
    );
    let fallback = headers(&[("X-Hub-Signature-256", &signature("test-secret", body))]);
    assert_eq!(
        receive_with(
            Provider::AppStoreConnect,
            Authentication::HmacSha256("test-secret"),
            &fallback,
            body
        )
        .unwrap_err(),
        WebhookError::Unauthorized
    );
    assert!(matches!(
        receive_with(
            Provider::AppStoreConnect,
            Authentication::HmacSha256(""),
            &h,
            body
        ),
        Err(WebhookError::InvalidConfiguration(_))
    ));
    assert!(matches!(
        receive_with(
            Provider::AppStoreConnect,
            Authentication::TelegramSecretToken("secret"),
            &h,
            body
        ),
        Err(WebhookError::InvalidConfiguration(_))
    ));
}

#[test]
fn background_relationships_are_direct_and_marketplace_events_need_no_instance() {
    let examples = examples();
    for payload in examples.iter().filter(|p| {
        p["data"]["type"]
            .as_str()
            .unwrap()
            .starts_with("backgroundAsset")
    }) {
        let parsed = parse(payload);
        let value = model(&parsed);
        assert!(value.data.version.is_none());
        let instance = value
            .data
            .relationships
            .value()
            .unwrap()
            .instance
            .value()
            .unwrap();
        assert!(instance.data.is_none());
        assert_eq!(
            instance.reference().id.value_deref(),
            payload["data"]["relationships"]["instance"]["id"].as_str()
        );
        assert_eq!(
            instance.self_url(),
            payload["data"]["relationships"]["instance"]["links"]["self"].as_str()
        );
        assert_eq!(
            parsed.to_message_content().url.as_deref(),
            instance.self_url()
        );
    }
    let payload = examples
        .iter()
        .find(|p| p["data"]["type"] == "alternativeDistributionTerritoryAvailabilityUpdated")
        .unwrap();
    let parsed = parse(payload);
    let value = model(&parsed);
    let relationships = value.data.relationships.value().unwrap();
    assert!(relationships.instance.is_none());
    let marketplace = relationships.marketplace_app.value().unwrap();
    assert_eq!(marketplace.reference().id.value_deref(), Some("9876543210"));
    assert_eq!(
        parsed.to_message_content().url.as_deref(),
        marketplace.self_url()
    );
    assert_eq!(
        value
            .data
            .attributes
            .value()
            .unwrap()
            .territories
            .value()
            .unwrap()[0],
        "DNK"
    );
}

#[test]
fn state_keys_are_not_conflated_and_build_upload_needs_no_timestamp_or_old_state() {
    let examples = examples();
    let find = |kind: &str| parse(examples.iter().find(|p| p["data"]["type"] == kind).unwrap());
    assert_eq!(
        find("appStoreVersionAppVersionStateUpdated")
            .to_message_content()
            .content,
        "PREPARE_FOR_SUBMISSION → READY_FOR_REVIEW"
    );
    assert_eq!(
        find("buildBetaDetailExternalBuildStateUpdated")
            .to_message_content()
            .content,
        "IN_BETA_REVIEW → BETA_APPROVED"
    );
    let upload = find("buildUploadStateUpdated");
    let attrs = model(&upload).data.attributes.value().unwrap();
    assert!(attrs.timestamp.is_none());
    assert!(attrs.old_state.is_none());
    assert!(attrs.new_value.is_none());
    assert_eq!(upload.to_message_content().content, "COMPLETE");
    let feedback = find("betaFeedbackScreenshotSubmissionCreated");
    assert!(
        model(&feedback)
            .data
            .attributes
            .value()
            .unwrap()
            .new_state
            .is_none()
    );
    assert_eq!(
        feedback.to_message_content().content,
        "Screenshot feedback created"
    );
}

#[test]
fn dedup_uses_notification_id_not_resource_id_or_delivery_metadata() {
    let mut payload = examples().remove(0);
    let first = parse(&payload);
    payload["data"]["id"] = json!("a-second-event-for-the-same-resource");
    let second = parse(&payload);
    assert_ne!(
        first.metadata.deduplication_key,
        second.metadata.deduplication_key
    );
    assert_eq!(
        model(&first).data.relationships,
        model(&second).data.relationships
    );
    assert_eq!(
        parse(&payload).metadata.deduplication_key,
        second.metadata.deduplication_key
    );
    payload["data"].as_object_mut().unwrap().remove("id");
    assert!(parse(&payload).metadata.deduplication_key.is_none());
}

#[test]
fn malformed_optional_fields_unknown_events_and_management_ping_are_not_control_contracts() {
    let payload = json!({"data":{"type":"futureNotification","id":"opaque-event","version":"future-version","attributes":{"timestamp":"2026-01-01T00:00:00Z","newState":{},"available":false,"territories":["ZZZ",false,"DNK"],"appId":"123","future":{"keep":true}},"relationships":{"instance":{"data":{"type":"futureResources","id":"not-a-uuid"},"links":{"self":"https://example.invalid/resource"}},"future":{"keep":true}}},"future":{"keep":true}});
    let parsed = parse(&payload);
    let value = model(&parsed);
    assert!(value.data.version.is_none());
    let attrs = value.data.attributes.value().unwrap();
    assert!(attrs.new_state.is_none());
    assert_eq!(attrs.available.into_value(), Some(false));
    assert_eq!(
        attrs.territories.value().unwrap().as_slice(),
        ["ZZZ", "DNK"]
    );
    assert_eq!(attrs.app_id.value_deref(), Some("123"));
    assert_eq!(
        value
            .data
            .relationships
            .value()
            .unwrap()
            .instance
            .value()
            .unwrap()
            .reference()
            .id
            .value_deref(),
        Some("not-a-uuid")
    );
    assert_eq!(
        serde_json::from_str::<Value>(&parsed.to_message_content().content).unwrap(),
        payload
    );
    // Apple documents webhookPings as a management API resource, not a complete
    // inbound Ping envelope. Never infer a Respond/ACK-only branch from its name.
    let unrecognized =
        parse(&json!({"data":{"type":"webhookPings","id":"example-management-resource"}}));
    assert_eq!(model(&unrecognized).data.event_type, "webhookPings");
    assert!(
        unrecognized
            .to_message_content()
            .content
            .contains("webhookPings")
    );
    for payload in [
        json!({}),
        json!({"data":[]}),
        json!({"data":null}),
        json!({"data":{"type":123}}),
        json!({"data":{"type":" "}}),
        json!({"signedPayload":"not-an-App-Store-Connect-notification"}),
    ] {
        let body = serde_json::to_vec(&payload).unwrap();
        let h = headers(&[("X-Apple-Signature", &signature("test-secret", &body))]);
        assert!(matches!(
            receive_with(
                Provider::AppStoreConnect,
                Authentication::HmacSha256("test-secret"),
                &h,
                &body,
            ),
            Err(WebhookError::InvalidPayload(_))
        ));
    }
    for body in [b"[]".as_slice(), b"null", b"invalid", b"payload=%7B%7D"] {
        let h = headers(&[("X-Apple-Signature", &signature("test-secret", body))]);
        assert!(matches!(
            receive_with(
                Provider::AppStoreConnect,
                Authentication::HmacSha256("test-secret"),
                &h,
                body
            ),
            Err(WebhookError::InvalidPayload(_))
        ));
    }
}
