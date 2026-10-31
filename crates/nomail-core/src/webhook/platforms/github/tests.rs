use crate::webhook::{test_support::*, *};
use serde_json::json;

#[test]
fn form_signature_covers_original_encoded_body() {
    let payload = json!({"action":"opened","issue":{"id":123,"title":"Bug","html_url":"https://example.com/issue"},"repository":{"full_name":"owner/repo"}});
    let body = serde_urlencoded::to_string([("payload", payload.to_string())]).unwrap();
    let signature = format!("sha256={}", hex::encode(sign(b"secret", body.as_bytes())));
    let h = headers(&[
        (
            "Content-Type",
            "application/x-www-form-urlencoded; charset=utf-8",
        ),
        ("X-GitHub-Event", "issues"),
        ("X-GitHub-Delivery", "delivery-1"),
        ("X-Hub-Signature-256", &signature),
    ]);
    let parsed = event(
        receive_with(
            Provider::Github,
            Authentication::HmacSha256("secret"),
            &h,
            body.as_bytes(),
        )
        .unwrap(),
    );
    let PlatformData::Github(github) = &parsed.data else {
        panic!()
    };
    assert_eq!(
        github.to_message_content(&parsed.metadata, &parsed.raw),
        parsed.to_message_content()
    );
    assert_eq!(parsed.raw.as_json(), Some(&payload));
    assert_eq!(parsed.provider(), Provider::Github);
    assert_eq!(
        github.repository.as_ref().unwrap().full_name.as_deref(),
        Some("owner/repo")
    );
    assert_eq!(
        parsed.to_message_content().title,
        "GitHub issues: opened: owner/repo"
    );
    assert_eq!(parsed.to_message_content().content, "Bug");
    assert_eq!(
        parsed.to_message_content().url.as_deref(),
        Some("https://example.com/issue")
    );
    assert_eq!(
        parsed.metadata().deduplication_key.as_ref().unwrap().value,
        "delivery-1"
    );
    assert!(
        receive_with(
            Provider::Github,
            Authentication::HmacSha256("secret"),
            &h,
            payload.to_string().as_bytes()
        )
        .is_err()
    );
}

#[test]
fn control_unknown_events_and_resource_ids() {
    let h = headers(&[("X-GitHub-Event", "ping")]);
    assert_eq!(
        response(receive_with(Provider::Github, Authentication::None, &h, b"{}").unwrap()).body,
        b"pong"
    );
    let h = headers(&[("X-GitHub-Event", "future_event")]);
    let parsed = event(
        receive_with(
            Provider::Github,
            Authentication::None,
            &h,
            br#"{"id":123,"custom":true}"#,
        )
        .unwrap(),
    );
    assert!(parsed.metadata().deduplication_key.is_none());
    assert!(parsed.to_message_content().content.contains("custom"));
    assert!(receive_with(Provider::Github, Authentication::None, &Vec::new(), b"{}").is_err());
}

#[test]
fn workflow_mapping() {
    let h = headers(&[("X-GitHub-Event", "workflow_run")]);
    let parsed = event(receive_with(Provider::Github, Authentication::None, &h, br#"{"repository":{"full_name":"owner/repo"},"workflow_run":{"name":"CI","conclusion":"failure","html_url":"https://example.com/run/1"}}"#).unwrap());
    assert_eq!(parsed.to_message_content().level, MessageLevel::Normal);
    assert_eq!(
        parsed.raw.as_json().unwrap()["workflow_run"]["conclusion"],
        "failure"
    );
    assert_eq!(parsed.to_message_content().content, "CI");
}

#[test]
fn issue_comments_are_normal_and_retain_structured_detail() {
    let h = headers(&[("X-GitHub-Event", "issue_comment")]);
    let payload = br#"{"action":"created","issue":{"number":123,"title":"Bug","labels":[{"name":"bug","color":"ff0000"}]},"comment":{"body":"  Reply  ","user":{"login":"alice"}},"workflow_run":{"conclusion":"failure"}}"#;
    let parsed = event(receive_with(Provider::Github, Authentication::None, &h, payload).unwrap());
    assert_eq!(parsed.to_message_content().level, MessageLevel::Normal);
    let PlatformData::Github(github) = &parsed.data else {
        panic!()
    };
    assert_eq!(github.issue.as_ref().unwrap().number, Some(123));
    assert_eq!(
        github.comment.as_ref().unwrap().body.as_deref(),
        Some("  Reply  ")
    );
    assert_eq!(
        github.issue.as_ref().unwrap().labels[0].name.as_deref(),
        Some("bug")
    );
}

#[test]
fn event_serialization_preserves_unknown_fields() {
    let parsed = event(
        receive_with(
            Provider::Github,
            Authentication::None,
            &headers(&[("X-GitHub-Event", "new_event")]),
            br#"{"unknown":{"preserved":true}}"#,
        )
        .unwrap(),
    );
    let encoded = serde_json::to_value(&parsed).unwrap();
    assert_eq!(encoded["data"]["provider"], "github");
    assert_eq!(encoded["raw"]["format"], "json");
    assert_eq!(encoded["raw"]["value"]["unknown"]["preserved"], true);
    let decoded: PlatformEvent = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded, parsed);
    assert_eq!(decoded.to_message_content(), parsed.to_message_content());
}

#[test]
fn malformed_signatures_and_duplicate_form_fields_are_rejected() {
    for signature in ["", "sha256=not-hex", "sha256=00", "sha1=123"] {
        let h = headers(&[
            ("X-Hub-Signature-256", signature),
            ("X-GitHub-Event", "push"),
        ]);
        assert_eq!(
            receive_with(
                Provider::Github,
                Authentication::HmacSha256("secret"),
                &h,
                b"{}"
            )
            .unwrap_err(),
            WebhookError::Unauthorized
        );
    }
    let h = headers(&[
        ("X-GitHub-Event", "push"),
        ("Content-Type", "application/x-www-form-urlencoded"),
    ]);
    assert!(matches!(
        receive_with(
            Provider::Github,
            Authentication::None,
            &h,
            b"payload=%7B%7D&payload=%7B%7D"
        ),
        Err(WebhookError::InvalidPayload(_))
    ));
}
