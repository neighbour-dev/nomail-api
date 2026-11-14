use crate::webhook::{test_support::*, *};
use serde_json::{Value, json};

fn delivery(payload: &Value, id: Option<&str>, attempt: &str) -> PlatformEvent {
    let body = serde_json::to_vec(payload).unwrap();
    let mut h = headers(&[
        (
            "X-Email-Platform-Signature",
            &hex::encode(sign(b"secret", &body)),
        ),
        ("X-Email-Platform-Event", "untrusted.header"),
        ("X-Email-Platform-Attempt", attempt),
    ]);
    if let Some(id) = id {
        h.push(("X-Email-Platform-Delivery".into(), id.into()));
    }
    event(
        receive_with(
            Provider::Mailflare,
            Authentication::HmacSha256("secret"),
            &h,
            &body,
        )
        .unwrap(),
    )
}

#[test]
fn native_events_test_deliveries_and_retry_identity() {
    for (kind, data, level) in [
        (
            "message.inbound",
            json!({"messageId":"msg_1","from":"sender@example.com","to":"inbox@example.com","cc":null,"subject":"Hello","threadId":"thread_1","spamScore":1,"spamVerdict":"clean"}),
            MessageLevel::Normal,
        ),
        (
            "message.outbound",
            json!({"messageId":"msg_2","providerMessageId":"provider_1","to":"inbox@example.com"}),
            MessageLevel::Normal,
        ),
        (
            "message.failed",
            json!({"messageId":"msg_3","error":"Synthetic send failure"}),
            MessageLevel::High,
        ),
    ] {
        let payload = json!({"type":kind,"data":data});
        let first = delivery(&payload, Some("whd_1"), "1");
        let retry = delivery(&payload, Some("whd_1"), "2");
        assert_eq!(first.metadata.event_type, kind);
        assert_eq!(
            first.metadata.deduplication_key,
            retry.metadata.deduplication_key
        );
        assert_eq!(
            first.metadata.deduplication_key.as_ref().unwrap().scope,
            "mailflare:delivery"
        );
        assert_eq!(first.to_message_content().level, level);
        if kind == "message.inbound" {
            assert_eq!(first.to_message_content().title, "Hello");
            assert!(first.to_message_content().summary.contains("Hello"));
        }
        if kind == "message.failed" {
            assert!(
                first
                    .to_message_content()
                    .summary
                    .contains("Synthetic send failure")
            );
        }
        assert_eq!(first.raw.as_json(), Some(&payload));
        assert_eq!(
            serde_json::to_value(first.data.as_ref().unwrap()).unwrap()["event"]["payload"],
            payload
        );
        assert_eq!(
            parse_raw(Provider::Mailflare, &first.raw).unwrap(),
            first.data.clone().unwrap()
        );
        assert!(
            !first
                .raw
                .header
                .iter()
                .any(|h| h.name.eq_ignore_ascii_case("x-email-platform-signature"))
        );
        assert_eq!(
            serde_json::from_value::<PlatformEvent>(serde_json::to_value(&first).unwrap()).unwrap(),
            first
        );
    }
    let test = delivery(
        &json!({"type":"message.inbound","test":true,"data":{"messageId":"test","subject":"Mailflare test delivery"}}),
        Some("whd_test"),
        "1",
    );
    assert_eq!(test.to_message_content().title, "Mailflare test delivery");
    assert!(test.to_message_content().summary.contains("test delivery"));
}

#[test]
fn unknown_fields_and_malformed_models_do_not_discard_signed_content() {
    let payload =
        json!({"type":"message.future","future":{"keep":true},"data":{"subject":42,"to":null}});
    let parsed = delivery(&payload, None, "1");
    assert_eq!(parsed.raw.as_json(), Some(&payload));
    assert!(parsed.metadata.deduplication_key.is_none());
    assert_eq!(
        parsed.to_message_content().title,
        "Mailflare message.future"
    );
    assert_eq!(
        serde_json::to_value(parsed.data.unwrap()).unwrap()["event"]["payload"],
        json!({"type":"message.future","data":{"to":null}})
    );
    let unmodeled = delivery(&json!(["future"]), None, "1");
    assert!(unmodeled.data.is_none());
    assert_eq!(unmodeled.raw.as_json(), Some(&json!(["future"])));
}

#[test]
fn sanitized_database_samples_keep_native_projection_and_retry_identity() {
    let fixtures: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/database_samples.json")).unwrap();
    assert_eq!(fixtures.len(), 4);
    for fixture in fixtures {
        let payload = &fixture["payload"];
        let body = serde_json::to_vec(payload).unwrap();
        let mut h: Vec<(String, String)> = fixture["headers"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(name, value)| (name.clone(), value.as_str().unwrap().into()))
            .collect();
        h.push((
            "X-Email-Platform-Signature".into(),
            hex::encode(sign(b"secret", &body)),
        ));
        let parsed = event(
            receive_with(
                Provider::Mailflare,
                Authentication::HmacSha256("secret"),
                &h,
                &body,
            )
            .unwrap(),
        );
        assert_eq!(
            serde_json::to_value(parsed.data.as_ref().unwrap()).unwrap()["event"]["payload"],
            *payload,
            "{}",
            fixture["name"]
        );
        assert_eq!(parsed.raw.as_json(), Some(payload));
        assert_eq!(
            parsed.to_message_content().title,
            payload["data"]["subject"]
        );
        assert_eq!(parsed.metadata.event_type, "message.inbound");
        assert_eq!(
            parsed.metadata.deduplication_key.as_ref().unwrap().value,
            fixture["headers"]["x-email-platform-delivery"]
        );
        let stored = RawWebhook::from_stored(
            parsed.raw.header.clone(),
            &parsed.raw.stored_payload(),
            parsed.raw.payload_content_type(),
        )
        .unwrap();
        assert_eq!(
            parse_raw(Provider::Mailflare, &stored).unwrap(),
            parsed.data.unwrap()
        );
        for (name, value) in &mut h {
            if name.eq_ignore_ascii_case("x-email-platform-attempt") {
                *value = "2".into();
            }
        }
        let retry = event(
            receive_with(
                Provider::Mailflare,
                Authentication::HmacSha256("secret"),
                &h,
                &body,
            )
            .unwrap(),
        );
        assert_eq!(parsed.metadata, retry.metadata);
        assert!(stored.header.iter().all(|header| {
            !header
                .name
                .eq_ignore_ascii_case("x-email-platform-signature")
        }));
    }
}

#[test]
fn native_field_presence_and_numeric_scores_are_preserved() {
    for data in [
        json!({}),
        json!({"subject":null,"spamScore":null,"to":""}),
        json!({"subject":"","spamScore":0,"from":null}),
        json!({"spamScore":1.5,"spamVerdict":"suspicious"}),
    ] {
        let payload = json!({"type":"message.inbound","test":false,"data":data});
        let parsed = delivery(&payload, Some("whd_fields"), "1");
        assert_eq!(
            serde_json::to_value(parsed.data.unwrap()).unwrap()["event"]["payload"],
            payload
        );
    }
    // Synthetic message IDs are shared across tests, but each delivery is distinct.
    let payload = json!({"type":"message.inbound","test":true,"data":{"messageId":"test"}});
    assert_ne!(
        delivery(&payload, Some("whd_a"), "1")
            .metadata
            .deduplication_key,
        delivery(&payload, Some("whd_b"), "1")
            .metadata
            .deduplication_key
    );
}

#[test]
fn authentication_requires_one_native_signature_over_exact_bytes() {
    let body = br#"{"type":"message.inbound","data":{}}"#;
    let signature = hex::encode(sign(b"secret", body));
    for value in ["", "00", "sha256=00", &"z".repeat(64)] {
        let h = headers(&[("X-Email-Platform-Signature", value)]);
        assert_eq!(
            receive_with(
                Provider::Mailflare,
                Authentication::HmacSha256("secret"),
                &h,
                body
            )
            .unwrap_err(),
            WebhookError::Unauthorized
        );
    }
    let h = headers(&[("X-Email-Platform-Signature", &signature)]);
    assert!(receive_with(Provider::Mailflare, Authentication::None, &h, body).is_err());
    assert!(
        receive_with(
            Provider::Mailflare,
            Authentication::HmacSha256(""),
            &h,
            body
        )
        .is_err()
    );
    assert!(
        receive_with(
            Provider::Mailflare,
            Authentication::HmacSha256("wrong"),
            &h,
            body
        )
        .is_err()
    );
    assert!(
        receive_with(
            Provider::Mailflare,
            Authentication::HmacSha256("secret"),
            &h,
            &[body.as_slice(), b" "].concat()
        )
        .is_err()
    );
    let mut duplicate = h.clone();
    duplicate.push(("x-email-platform-signature".into(), signature.clone()));
    assert!(
        receive_with(
            Provider::Mailflare,
            Authentication::HmacSha256("secret"),
            &duplicate,
            body
        )
        .is_err()
    );
    let compatibility = headers(&[("X-Hub-Signature-256", &format!("sha256={signature}"))]);
    assert!(
        receive_with(
            Provider::Mailflare,
            Authentication::HmacSha256("secret"),
            &compatibility,
            body
        )
        .is_err()
    );
    let invalid_json = b"{";
    let h = headers(&[(
        "X-Email-Platform-Signature",
        &hex::encode(sign(b"secret", invalid_json)),
    )]);
    assert!(matches!(
        receive_with(
            Provider::Mailflare,
            Authentication::HmacSha256("secret"),
            &h,
            invalid_json
        ),
        Err(WebhookError::InvalidPayload(_))
    ));
}
