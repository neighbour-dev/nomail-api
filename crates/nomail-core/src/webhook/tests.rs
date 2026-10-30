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
        (" ERROR ", MessageLevel::Critical),
        ("warn", MessageLevel::Warning),
        ("pass", MessageLevel::Success),
        ("info", MessageLevel::Normal),
    ] {
        assert_eq!(MessageLevel::from_label(label), expected);
    }
}
