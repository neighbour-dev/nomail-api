use crate::webhook::{test_support::*, *};

#[test]
fn signed_json_and_text_mapping() {
    let body = br#"{"id":"event-1","title":"Disk alert","content":"Usage is 92%","level":"urgent","extra":{"kept":true}}"#;
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
    assert_eq!(parsed.to_message_content().title, "Disk alert");
    assert_eq!(parsed.to_message_content().level, MessageLevel::Urgent);
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
        ("X-NoMail-Level", "low"),
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
        parsed.to_message_content(),
        MessageContent {
            title: "Backup".into(),
            content: "Backup done".into(),
            source: Some("NAS".into()),
            level: MessageLevel::Low,
            url: None
        }
    );
    assert!(parsed.metadata().deduplication_key.is_none());
    assert_eq!(parsed.raw.as_text(), Some(" Backup done "));
    let PlatformData::Generic(GenericEvent::Text(fields)) = &parsed.data else {
        panic!()
    };
    assert_eq!(fields.title.as_deref(), Some("Backup"));
    assert!(receive_with(Provider::Generic, Authentication::None, &h, b" ").is_err());
}

#[test]
fn json_protocol_fields_are_not_text_header_defaults() {
    let h = headers(&[
        ("X-NoMail-Title", "header title"),
        ("X-NoMail-Level", "urgent"),
    ]);
    let body = br#"{"eventType":"backup","message":"done","unknown":{"retained":true}}"#;
    let parsed = event(receive_with(Provider::Generic, Authentication::None, &h, body).unwrap());
    let PlatformData::Generic(GenericEvent::Json(fields)) = &parsed.data else {
        panic!()
    };
    assert_eq!(fields.message.as_deref(), Some("done"));
    assert!(fields.title.is_none());
    assert_eq!(parsed.to_message_content().title, "backup");
    assert_eq!(parsed.to_message_content().level, MessageLevel::Normal);
    assert_eq!(parsed.raw.as_json().unwrap()["unknown"]["retained"], true);
}

#[test]
fn unreadable_text_is_rejected_instead_of_silently_corrupted() {
    assert!(matches!(
        receive_with(
            Provider::Generic,
            Authentication::None,
            &Vec::new(),
            &[0xff, 0xfe]
        ),
        Err(WebhookError::InvalidPayload(_))
    ));
}

#[test]
fn legacy_outcome_labels_are_not_priority_aliases_and_remain_in_platform_data() {
    for label in ["success", "warning", "critical", "error", "failure"] {
        let body = serde_json::json!({"title":"Result","level":label}).to_string();
        let parsed = event(
            receive_with(
                Provider::Generic,
                Authentication::None,
                &Vec::new(),
                body.as_bytes(),
            )
            .unwrap(),
        );
        assert_eq!(parsed.to_message_content().level, MessageLevel::Normal);
        let PlatformData::Generic(GenericEvent::Json(fields)) = &parsed.data else {
            panic!()
        };
        assert_eq!(fields.level.as_deref(), Some(label));
        assert_eq!(parsed.raw.as_json().unwrap()["level"], label);
    }
}
