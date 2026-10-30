use crate::webhook::{test_support::*, *};
use base64::{Engine, engine::general_purpose::STANDARD};

#[test]
fn legacy_token_and_custom_payload() {
    let h = headers(&[
        ("X-Gitlab-Event", "Merge Request Hook"),
        ("X-Gitlab-Token", "token"),
        ("Idempotency-Key", "msg-1"),
        ("X-Gitlab-Event-UUID", "not-the-key"),
    ]);
    let parsed = event(
        receive_with(
            Provider::Gitlab,
            Authentication::GitlabToken("token"),
            &h,
            br#"{"project_name":"custom template"}"#,
        )
        .unwrap(),
    );
    assert_eq!(
        parsed.metadata().deduplication_key.as_ref().unwrap().value,
        "msg-1"
    );
    assert!(parsed.to_message().content.contains("custom template"));
}

#[test]
fn signing_rotation_timestamp_and_no_auth_downgrade() {
    let body = b"{}";
    let secret = format!("whsec_{}", STANDARD.encode(b"signing-key"));
    let signed = format!("message-1.{NOW}.{{}}");
    let signature = format!(
        "v1,{}",
        STANDARD.encode(sign(b"signing-key", signed.as_bytes()))
    );
    let mut h = headers(&[
        ("X-Gitlab-Event", "Push Hook"),
        ("webhook-id", "message-1"),
        ("webhook-timestamp", &NOW.to_string()),
        ("webhook-signature", "v1,aW52YWxpZA=="),
        ("webhook-signature", &signature),
    ]);
    assert!(
        receive_with(
            Provider::Gitlab,
            Authentication::GitlabSigningToken(&secret),
            &h,
            body
        )
        .is_ok()
    );
    h.retain(|(name, _)| name != "webhook-signature");
    h.push(("X-Gitlab-Token".into(), secret.clone()));
    assert_eq!(
        receive_with(
            Provider::Gitlab,
            Authentication::GitlabSigningToken(&secret),
            &h,
            body
        )
        .unwrap_err(),
        WebhookError::Unauthorized
    );
    h.push(("webhook-signature".into(), signature));
    h.iter_mut()
        .find(|(name, _)| name == "webhook-timestamp")
        .unwrap()
        .1 = (NOW - 301).to_string();
    assert_eq!(
        receive_with(
            Provider::Gitlab,
            Authentication::GitlabSigningToken(&secret),
            &h,
            body
        )
        .unwrap_err(),
        WebhookError::Unauthorized
    );
}
