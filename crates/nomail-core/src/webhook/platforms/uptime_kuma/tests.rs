use super::*;
use crate::webhook::{test_support::*, *};
use serde_json::{Value, json};

fn parse(payload: &Value) -> PlatformEvent {
    let body = serde_json::to_vec(payload).unwrap();
    let h = headers(&[]);
    let parsed =
        event(receive_with(Provider::UptimeKuma, Authentication::None, &h, &body).unwrap());
    assert_eq!(parsed.raw.as_json(), Some(payload));
    assert_eq!(
        parse_raw(Provider::UptimeKuma, &parsed.raw).ok(),
        parsed.data
    );
    let restored: PlatformEvent =
        serde_json::from_value(serde_json::to_value(&parsed).unwrap()).unwrap();
    assert_eq!(restored, parsed);
    parsed
}

fn model(parsed: &PlatformEvent) -> &UptimeKumaWebhook {
    let Some(PlatformData::UptimeKuma(value)) = &parsed.data else {
        panic!("missing Uptime Kuma model");
    };
    value
}

#[test]
fn synthetic_upstream_shaped_notifications_keep_fields_and_do_not_guess_dedup_keys() {
    let fixtures: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/notifications.json")).unwrap();
    for fixture in fixtures {
        let payload = &fixture["payload"];
        let first = parse(payload);
        let second = parse(payload);
        assert_eq!(first.provider(), Provider::UptimeKuma);
        assert_eq!(first.metadata.event_type, fixture["name"].as_str().unwrap());
        assert!(first.metadata.deduplication_key.is_none());
        assert!(second.metadata.deduplication_key.is_none());
        assert_eq!(
            serde_json::to_value(&model(&first).payload).unwrap(),
            *payload
        );
        assert_eq!(
            serde_json::to_value(&model(&first).header).unwrap(),
            json!({})
        );
        let content = first.to_message_content();
        assert_eq!(content.summary, summarize(payload["msg"].as_str().unwrap()));
        assert_eq!(content.level, MessageLevel::Normal);
        assert_eq!(content.url.as_deref(), payload["monitor"]["url"].as_str());
        let stored = RawWebhook::from_stored(
            first.raw.header.clone(),
            &first.raw.stored_payload(),
            first.raw.payload_content_type(),
        )
        .unwrap();
        assert_eq!(
            parse_raw(Provider::UptimeKuma, &stored).unwrap(),
            first.data.unwrap()
        );
    }
}

#[test]
fn numeric_statuses_are_not_coerced_or_confused_with_priority() {
    for (status, expected) in [
        (json!(0), "down"),
        (json!(1), "up"),
        (json!(2), "pending"),
        (json!(3), "maintenance"),
        (json!(99), "notification"),
        (json!("0"), "notification"),
        (json!(false), "notification"),
        (json!(null), "notification"),
    ] {
        let parsed = parse(&json!({"heartbeat":{"status":status,"msg":"原始状态 🚀"},
            "monitor":{"name":"API","url":"https://example.invalid"}}));
        assert_eq!(parsed.metadata.event_type, expected);
        let content = parsed.to_message_content();
        assert_eq!(content.title, format!("Uptime Kuma: API ({expected})"));
        assert_eq!(content.summary, "原始状态 🚀");
        assert_eq!(content.level, MessageLevel::Normal);
    }
}

#[test]
fn payload_presence_unknown_fields_and_wrong_optional_types_are_lossless_in_raw() {
    for payload in [
        json!({}),
        json!({"heartbeat":null,"monitor":null,"msg":null}),
        json!({"heartbeat":{},"monitor":{},"msg":""}),
        json!({"heartbeat":{"status":0,"ping":0,"duration":0,"important":false,"response":""},
            "monitor":{"id":0,"name":"","active":false},"msg":""}),
    ] {
        assert_eq!(
            serde_json::to_value(&model(&parse(&payload)).payload).unwrap(),
            payload
        );
    }
    let payload = json!({"heartbeat":{"status":"down","ping":"12","monitorID":false,
        "important":true,"msg":"valid","future":{"keep":true}},
        "monitor":{"name":" API ","port":"443","future":[1,null]},"msg":false,"future":true});
    let parsed = parse(&payload);
    let typed = model(&parsed);
    let heartbeat = typed.payload.heartbeat.value().unwrap();
    assert!(heartbeat.status.is_none());
    assert!(heartbeat.ping.is_none());
    assert!(heartbeat.monitor_id.is_none());
    assert_eq!(heartbeat.important.into_value(), Some(true));
    assert_eq!(
        typed.payload.monitor.value().unwrap().name.value_deref(),
        Some(" API ")
    );
    assert_eq!(parsed.to_message_content().summary, "valid");
    for payload in [json!(null), json!([]), json!("text")] {
        let parsed = parse(&payload);
        assert!(parsed.data.is_none());
        assert!(parsed.to_message_content().summary.is_empty());
    }
}

#[test]
fn native_webhook_needs_no_authentication_and_does_not_validate_optional_headers() {
    let body = br#"{"msg":"test","heartbeat":null,"monitor":null}"#;
    for h in [
        headers(&[]),
        headers(&[("Authorization", "Bearer optional-credential")]),
        headers(&[("X-Unknown", "retained")]),
    ] {
        let parsed =
            event(receive_with(Provider::UptimeKuma, Authentication::None, &h, body).unwrap());
        assert_eq!(parsed.to_message_content().summary, "test");
        assert!(
            !serde_json::to_string(&parsed)
                .unwrap()
                .contains("optional-credential")
        );
    }
    let h = headers(&[]);
    for auth in [
        Authentication::HmacSha256("test-secret"),
        Authentication::TelegramSecretToken("test-secret"),
    ] {
        assert!(matches!(
            receive_with(Provider::UptimeKuma, auth, &h, body),
            Err(WebhookError::InvalidConfiguration(_))
        ));
    }
}

#[test]
fn only_json_payloads_are_decoded_and_no_message_is_invented() {
    let h = headers(&[]);
    for body in [
        b"invalid".as_slice(),
        b"data=%7B%22msg%22%3A%22test%22%7D",
        b"--multipart-boundary\r\nContent-Disposition: form-data; name=\"data\"\r\n\r\n{}",
    ] {
        assert!(matches!(
            receive_with(Provider::UptimeKuma, Authentication::None, &h, body),
            Err(WebhookError::InvalidPayload(_))
        ));
    }
    let parsed =
        parse(&json!({"msg":" \n ","heartbeat":{"msg":"fallback text"},"monitor":{"name":" "}}));
    assert_eq!(
        parsed.to_message_content().title,
        "Uptime Kuma: notification"
    );
    assert_eq!(parsed.to_message_content().summary, "fallback text");
    assert!(
        parse(&json!({"future":"not a summary"}))
            .to_message_content()
            .summary
            .is_empty()
    );
}
