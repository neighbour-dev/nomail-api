use crate::webhook::{test_support::*, *};
use serde_json::{Value, json};

fn slack_headers(body: &[u8], content_type: &str) -> Vec<(String, String)> {
    let mut signed = format!("v0:{NOW}:").into_bytes();
    signed.extend_from_slice(body);
    let signature = format!("v0={}", hex::encode(sign(b"slack-secret", &signed)));
    headers(&[
        ("Content-Type", content_type),
        ("X-Slack-Request-Timestamp", &NOW.to_string()),
        ("X-Slack-Signature", &signature),
    ])
}

#[test]
fn events_interactions_and_slash_commands() {
    let events = br#"{"type":"event_callback","event_id":"Ev1","team_id":"T1","event":{"type":"message","text":"hello"}}"#;
    let interaction = serde_urlencoded::to_string([(
        "payload",
        r#"{"type":"block_actions","team":{"id":"T1"},"user":{"id":"U1"},"actions":[{"value":"approve"}]}"#,
    )])
    .unwrap();
    let command = b"team_id=T1&command=%2Fexample&text=hello";
    for (body, content_type, kind) in [
        (events.as_slice(), "application/json", 0),
        (
            interaction.as_bytes(),
            "application/x-www-form-urlencoded",
            1,
        ),
        (command.as_slice(), "application/x-www-form-urlencoded", 2),
    ] {
        let parsed = event(
            receive_with(
                Provider::Slack,
                Authentication::SlackSigningSecret("slack-secret"),
                &slack_headers(body, content_type),
                body,
            )
            .unwrap(),
        );
        let PlatformData::Slack(webhook) = &parsed.data else {
            panic!()
        };
        match kind {
            0 => assert_eq!(
                webhook.payload.event_type.value_deref(),
                Some("event_callback")
            ),
            1 => {
                assert_eq!(
                    webhook.payload.event_type.value_deref(),
                    Some("block_actions")
                );
                assert_eq!(
                    webhook.payload.team.value().unwrap().id.value_deref(),
                    Some("T1")
                );
                assert_eq!(
                    webhook.payload.user.value().unwrap().id.value_deref(),
                    Some("U1")
                );
                assert!(webhook.payload.team_id.is_none());
                assert!(webhook.payload.user_id.is_none());
            }
            2 => assert_eq!(webhook.payload.command.value_deref(), Some("/example")),
            _ => unreachable!(),
        }
        assert!(!parsed.to_message_content().content.is_empty());
        match kind {
            0 => assert_eq!(parsed.raw.as_json().unwrap()["event"]["text"], "hello"),
            1 => assert_eq!(
                parsed.raw.as_json().unwrap()["actions"][0]["value"],
                "approve"
            ),
            2 => {
                let EventContent::Form(fields) = &parsed.raw else {
                    panic!()
                };
                assert!(fields.contains(&("command".into(), "/example".into())));
                assert!(!parsed.raw.to_readable_string().contains("%2F"));
            }
            _ => unreachable!(),
        }
        if kind == 0 {
            assert_eq!(
                parsed.metadata().deduplication_key.as_ref().unwrap().value,
                "Ev1"
            );
        }
    }
}

#[test]
fn verification_and_rate_limit_are_not_messages() {
    let body = br#"{"type":"url_verification","challenge":"value"}"#;
    let h = slack_headers(body, "application/json");
    let reply = response(
        receive_with(
            Provider::Slack,
            Authentication::SlackSigningSecret("slack-secret"),
            &h,
            body,
        )
        .unwrap(),
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&reply.body).unwrap(),
        json!({"challenge":"value"})
    );
    let body = br#"{"type":"app_rate_limited"}"#;
    assert!(matches!(
        receive_with(
            Provider::Slack,
            Authentication::SlackSigningSecret("slack-secret"),
            &slack_headers(body, "application/json"),
            body
        )
        .unwrap(),
        ReceiveOutcome::Ignored { .. }
    ));
}

#[test]
fn timestamp_is_enforced() {
    let body = br#"{"type":"event_callback","event":{"type":"message"}}"#;
    let mut h = slack_headers(body, "application/json");
    h.iter_mut()
        .find(|(name, _)| name == "X-Slack-Request-Timestamp")
        .unwrap()
        .1 = (NOW - 301).to_string();
    assert_eq!(
        receive_with(
            Provider::Slack,
            Authentication::SlackSigningSecret("slack-secret"),
            &h,
            body
        )
        .unwrap_err(),
        WebhookError::Unauthorized
    );
}
