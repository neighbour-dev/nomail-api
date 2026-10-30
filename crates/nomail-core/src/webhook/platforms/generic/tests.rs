use crate::webhook::{test_support::*, *};

#[test]
fn signed_json_and_text_mapping() {
    let body = br#"{"id":"event-1","title":"Disk alert","content":"Usage is 92%","level":"critical","extra":{"kept":true}}"#;
    let signature = format!("sha256={}", hex::encode(sign(b"secret", body)));
    let h = headers(&[("X-NoMail-Signature", &signature)]);
    let parsed = event(
        receive_with(
            Provider::Generic,
            Authentication::HmacSha256("secret"),
            &h,
            body,
        )
        .unwrap(),
    );
    assert_eq!(
        parsed.metadata().deduplication_key.as_ref().unwrap().value,
        "event-1"
    );
    assert_eq!(parsed.to_message().title, "Disk alert");
    assert_eq!(parsed.to_message().level, MessageLevel::Critical);
    assert!(
        serde_json::to_value(&parsed)
            .unwrap()
            .to_string()
            .contains("kept")
    );
    assert_eq!(
        receive_with(
            Provider::Generic,
            Authentication::HmacSha256("secret"),
            &h,
            b"tampered"
        )
        .unwrap_err(),
        WebhookError::Unauthorized
    );

    let h = headers(&[
        ("X-NoMail-Title", "Backup"),
        ("X-NoMail-Level", "passed"),
        ("X-NoMail-Source", "NAS"),
    ]);
    let parsed = event(
        receive_with(
            Provider::Generic,
            Authentication::None,
            &h,
            b" Backup done ",
        )
        .unwrap(),
    );
    assert_eq!(
        parsed.to_message(),
        IncomingMessage {
            title: "Backup".into(),
            content: "Backup done".into(),
            source: Some("NAS".into()),
            level: MessageLevel::Success,
            url: None
        }
    );
    assert!(parsed.metadata().deduplication_key.is_none());
    assert!(receive_with(Provider::Generic, Authentication::None, &h, b" ").is_err());
}
