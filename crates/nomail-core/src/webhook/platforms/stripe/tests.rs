use crate::webhook::{test_support::*, *};

fn stripe_headers(body: &[u8], time: i64) -> Vec<(String, String)> {
    let mut signed = format!("{time}.").into_bytes();
    signed.extend_from_slice(body);
    let signature = hex::encode(sign(b"whsec_literal-not-base64", &signed));
    headers(&[(
        "Stripe-Signature",
        &format!("t={time},v1=bad,v1={signature}"),
    )])
}

#[test]
fn snapshot_thin_and_rotating_signatures() {
    let snapshot = br#"{"id":"evt_1","object":"event","type":"payment_intent.succeeded","api_version":"2024-06-20","created":1710000000,"data":{"object":{"id":"pi_1"}}}"#;
    let thin = br#"{"id":"evt_2","object":"v2.core.event","type":"v2.core.account.updated","created":"2026-03-09T13:00:28Z","related_object":{"id":"acct_1","url":"/v2/accounts/acct_1"}}"#;
    for (body, is_thin) in [(snapshot.as_slice(), false), (thin.as_slice(), true)] {
        let h = stripe_headers(body, NOW);
        let parsed = event(
            receive_with(
                Provider::Stripe,
                Authentication::StripeSigningSecret("whsec_literal-not-base64"),
                &h,
                body,
            )
            .unwrap(),
        );
        assert_eq!(
            matches!(parsed.data, PlatformData::Stripe(StripeEvent::Thin(_))),
            is_thin
        );
        if is_thin {
            assert!(parsed.to_message().content.contains("details not fetched"));
            assert!(parsed.to_message().url.is_none());
        } else {
            assert_eq!(parsed.to_message().level, MessageLevel::Success);
        }
        assert!(
            receive_with(
                Provider::Stripe,
                Authentication::StripeSigningSecret("whsec_literal-not-base64"),
                &h,
                b"{}"
            )
            .is_err()
        );
    }
}

#[test]
fn timestamp_window_checks_past_and_future() {
    let body = br#"{"id":"evt_1","type":"test","object":"event"}"#;
    for time in [NOW - 301, NOW + 301] {
        assert_eq!(
            receive_with(
                Provider::Stripe,
                Authentication::StripeSigningSecret("whsec_literal-not-base64"),
                &stripe_headers(body, time),
                body
            )
            .unwrap_err(),
            WebhookError::Unauthorized
        );
    }
    assert!(
        receive_with(
            Provider::Stripe,
            Authentication::StripeSigningSecret("whsec_literal-not-base64"),
            &stripe_headers(body, NOW - 300),
            body
        )
        .is_ok()
    );
}

#[test]
fn unknown_envelopes_are_rejected() {
    assert!(matches!(
        receive_with(
            Provider::Stripe,
            Authentication::None,
            &Vec::new(),
            br#"{"id":"evt_1","type":"test","object":"unknown"}"#
        ),
        Err(WebhookError::Unsupported(_))
    ));
}
