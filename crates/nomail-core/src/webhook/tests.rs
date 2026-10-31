//! Cross-platform API contracts. Platform cases and fixtures live in platforms/*/tests.rs.
use super::{test_support::*, *};

#[test]
fn configuration_is_explicit_and_headers_are_case_insensitive() {
    assert_eq!(" GitHub ".parse::<Provider>().unwrap(), Provider::Github);
    assert!("unknown".parse::<Provider>().is_err());
    let h = headers(&[("X-Repeated", "one"), ("x-repeated", "two")]);
    assert_eq!(h.values("X-REPEATED"), ["one", "two"]);
    assert!(matches!(
        receive_with(
            Provider::Github,
            Authentication::TelegramSecretToken("secret"),
            &h,
            b"{}"
        ),
        Err(WebhookError::InvalidConfiguration(_))
    ));
    assert!(matches!(
        receive_with(Provider::Github, Authentication::HmacSha256(""), &h, b"{}"),
        Err(WebhookError::InvalidConfiguration(_))
    ));
    let config = WebhookConfig::new(Provider::Generic, Authentication::None, -1);
    assert!(matches!(
        receive(
            &config,
            &WebhookRequest {
                headers: &h,
                body: b"{}"
            }
        ),
        Err(WebhookError::InvalidConfiguration(_))
    ));
}

#[test]
fn invalid_json_objects_are_rejected() {
    for provider in [
        Provider::Gitlab,
        Provider::Stripe,
        Provider::Slack,
        Provider::Feishu,
        Provider::Telegram,
        Provider::Shopify,
    ] {
        for body in [b"null".as_slice(), b"[]", b"invalid"] {
            assert!(receive_with(provider, Authentication::None, &Vec::new(), body).is_err());
        }
    }
}

#[test]
fn normalized_levels_are_stable() {
    for (label, expected) in [
        (" LOW ", MessageLevel::Low),
        ("normal", MessageLevel::Normal),
        ("High", MessageLevel::High),
        (" URGENT ", MessageLevel::Urgent),
        ("error", MessageLevel::Normal),
        ("warning", MessageLevel::Normal),
        ("critical", MessageLevel::Normal),
        ("success", MessageLevel::Normal),
        ("failed", MessageLevel::Normal),
        ("info", MessageLevel::Normal),
    ] {
        assert_eq!(MessageLevel::from_label(label), expected);
    }
    for label in ["low", "normal", "high", "urgent"] {
        let level: MessageLevel = label.parse().unwrap();
        assert_eq!(
            serde_json::to_string(&level).unwrap(),
            format!("\"{label}\"")
        );
        assert_eq!(level.as_str(), label);
    }
    assert!("critical".parse::<MessageLevel>().is_err());
}

#[test]
fn readable_content_has_explicit_format_and_is_serializable() {
    let contents = [
        EventContent::Json(serde_json::json!({"text":"你好","unknown":[1,2]})),
        EventContent::Text(" untouched text\n".into()),
        EventContent::Form(vec![
            ("command".into(), "/notify".into()),
            ("tag".into(), "a".into()),
            ("tag".into(), "b".into()),
        ]),
    ];
    for content in contents {
        let encoded = serde_json::to_value(&content).unwrap();
        let decoded: EventContent = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded, content);
        assert_eq!(decoded.to_readable_string(), content.to_readable_string());
    }
    let json = EventContent::Json(serde_json::json!({"text":"你好"}));
    assert!(json.to_readable_string().contains("你好"));
    assert_eq!(json.media_type(), "application/json");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&json.to_readable_string()).unwrap(),
        *json.as_json().unwrap()
    );
}

#[test]
fn event_envelope_roundtrips_without_losing_unknown_content() {
    let h = headers(&[
        ("X-GitHub-Event", "issues"),
        ("X-GitHub-Delivery", "delivery-1"),
    ]);
    let parsed = event(
        receive_with(
            Provider::Github,
            Authentication::None,
            &h,
            br#"{"action":"opened","issue":{"title":"Issue"},"unknown":{"keep":true}}"#,
        )
        .unwrap(),
    );
    let before = parsed.clone();
    let message = parsed.to_message_content();
    assert_eq!(parsed, before);
    let decoded: PlatformEvent =
        serde_json::from_str(&serde_json::to_string(&parsed).unwrap()).unwrap();
    assert_eq!(decoded, parsed);
    assert_eq!(decoded.to_message_content(), message);
    assert_eq!(
        decoded.metadata.deduplication_key.as_ref().unwrap().value,
        "delivery-1"
    );
    assert_eq!(decoded.raw.as_json().unwrap()["unknown"]["keep"], true);
}
