use crate::webhook::{test_support::*, *};

#[test]
fn update_types_and_non_text_messages() {
    let h = headers(&[("X-TELEGRAM-BOT-API-SECRET-TOKEN", "token")]);
    for (body, kind, expected) in [
        (
            br#"{"update_id":1,"message":{"photo":[{"file_id":"photo-1"}]}}"#.as_slice(),
            "message",
            "photo-1",
        ),
        (
            br#"{"update_id":2,"callback_query":{"id":"q1","data":"approve"}}"#,
            "callback_query",
            "approve",
        ),
        (
            br#"{"update_id":3,"future_update":{"new_field":true}}"#,
            "future_update",
            "new_field",
        ),
    ] {
        let parsed = event(
            receive_with(
                Provider::Telegram,
                Authentication::TelegramSecretToken("token"),
                &h,
                body,
            )
            .unwrap(),
        );
        assert_eq!(parsed.metadata().event_type, kind);
        assert!(parsed.to_message().content.contains(expected));
    }
    assert!(
        receive_with(
            Provider::Telegram,
            Authentication::TelegramSecretToken("token"),
            &h,
            br#"{"update_id":1,"message":{},"callback_query":{}}"#
        )
        .is_err()
    );
    assert!(
        receive_with(
            Provider::Telegram,
            Authentication::TelegramSecretToken("token"),
            &h,
            br#"{"ok":true,"result":{}}"#
        )
        .is_err()
    );
    assert_eq!(
        receive_with(
            Provider::Telegram,
            Authentication::TelegramSecretToken("other"),
            &h,
            br#"{"update_id":1,"message":{"text":"hello"}}"#
        )
        .unwrap_err(),
        WebhookError::Unauthorized
    );
}
