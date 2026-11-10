use super::models::TelegramValue;
use super::*;
use crate::webhook::{test_support::*, *};
use serde_json::{Value, json};

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
        assert!(parsed.to_message_content().content.contains(expected));
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

fn parse(payload: &Value) -> PlatformEvent {
    event(
        receive_with(
            Provider::Telegram,
            Authentication::None,
            &headers(&[]),
            &serde_json::to_vec(payload).unwrap(),
        )
        .unwrap(),
    )
}

#[test]
fn documented_update_and_nested_field_values_survive_typed_projection() {
    // Synthetic, sanitized examples independently assembled from the linked
    // Bot API definitions, not captured traffic or model round-trip inputs.
    // Check every input field/value, rather than merely accepting the envelope.
    let fixtures: Vec<Value> = serde_json::from_str(include_str!("fixtures/updates.json")).unwrap();
    let mut kinds = std::collections::BTreeSet::new();
    for payload in fixtures {
        let parsed = parse(&payload);
        kinds.insert(parsed.metadata.event_type.clone());
        assert_eq!(parsed.raw.as_json(), Some(&payload));
        let PlatformData::Telegram(data) = &parsed.data else {
            panic!("Telegram model")
        };
        assert_eq!(data.update_id, payload["update_id"].as_i64().unwrap());
        assert_eq!(
            serde_json::to_value(data).unwrap(),
            payload,
            "update {} lost or changed a documented field",
            data.update_id
        );
        let snapshot = serde_json::to_value(&parsed).unwrap();
        let restored: PlatformEvent = serde_json::from_value(snapshot).unwrap();
        assert_eq!(restored, parsed);
    }
    assert_eq!(
        kinds.len(),
        29,
        "all documented Update branches must be exercised"
    );
}

#[test]
fn text_entities_and_ids_remain_exact_in_native_payload_structure() {
    let payload = json!({"update_id":0,"message":{
        "message_id":0,"date":1794268800,
        "chat":{"id":-1001234567890_i64,"type":"supergroup"},
        "text":" 😀 hello \n", "entities":[{"type":"bold","offset":4,"length":5}],
        "caption":" caption \n"
    }});
    let parsed = parse(&payload);
    let PlatformData::Telegram(data) = &parsed.data else {
        panic!()
    };
    assert_eq!(data.update_id, 0);
    assert!(data.callback_query.is_none());
    let message = data.message.as_ref().unwrap();
    assert_eq!(message.chat.as_ref().unwrap().id, Some(-1001234567890));
    assert_eq!(message.message_id, Some(0));
    assert_eq!(message.text.as_deref(), Some(" 😀 hello \n"));
    assert_eq!(message.caption.as_deref(), Some(" caption \n"));
    let entity = &message.entities.as_ref().unwrap()[0];
    let units: Vec<_> = message.text.as_ref().unwrap().encode_utf16().collect();
    let start = entity.offset.unwrap() as usize;
    let end = start + entity.length.unwrap() as usize;
    assert_eq!(String::from_utf16(&units[start..end]).unwrap(), "hello");
    let key = parsed.metadata.deduplication_key.as_ref().unwrap();
    assert_eq!(key.value, "0");
    assert_eq!(key.scope, "telegram:bot-update");
}

#[test]
fn union_discriminators_select_the_correct_variant() {
    for (date, inaccessible) in [(0, true), (1794268800, false)] {
        let parsed = parse(&json!({"update_id":1,"callback_query":{
            "id":"000123", "chat_instance":"-000456", "data":" approve ",
            "message":{"message_id":0,"date":date,"chat":{"id":-1001234567890_i64,"type":"supergroup"}}
        }}));
        let PlatformData::Telegram(data) = parsed.data else {
            panic!()
        };
        let callback = data.callback_query.unwrap();
        assert_eq!(callback.id.as_deref(), Some("000123"));
        let message = serde_json::to_value(&callback.message).unwrap();
        assert_eq!(message["chat"]["id"], -1001234567890_i64);
        assert_eq!(message["message_id"], 0);
        assert_eq!(callback.chat_instance.as_deref(), Some("-000456"));
        assert_eq!(callback.data.as_deref(), Some(" approve "));
        assert_eq!(
            matches!(
                callback.message.as_deref(),
                Some(TelegramMaybeInaccessibleMessage::InaccessibleMessage(_))
            ),
            inaccessible
        );
        assert_eq!(
            matches!(
                callback.message.as_deref(),
                Some(TelegramMaybeInaccessibleMessage::Message(_))
            ),
            !inaccessible
        );
    }
    assert!(matches!(
        TelegramChatMember::read(&json!({"status":"kicked","until_date":0})),
        Some(TelegramChatMember::Banned(_))
    ));
    assert!(matches!(
        TelegramChatMember::read(&json!({"status":"creator","is_anonymous":true})),
        Some(TelegramChatMember::Owner(_))
    ));
    assert!(matches!(
        TelegramReactionType::read(&json!({"type":"paid"})),
        Some(TelegramReactionType::Paid(_))
    ));
    assert!(matches!(
        TelegramMessageOrigin::read(&json!({"type":"hidden_user","sender_user_name":"Alice"})),
        Some(TelegramMessageOrigin::HiddenUser(_))
    ));
    assert!(matches!(
        TelegramChatBoostSource::read(&json!({"source":"gift_code"})),
        Some(TelegramChatBoostSource::GiftCode(_))
    ));
}

#[test]
fn malformed_fields_are_local_and_future_data_has_a_lossless_fallback() {
    let payload = json!({"update_id":1,"future_metadata":true,"message":{
        "message_id":0,"date":1794268800,"from":false,
        "chat":{"id":-1001234567890_i64,"type":"supergroup","title":123},
        "text":"", "caption":123,"photo":[null,{"file_id":"","width":"wrong","height":90}],
        "entities":[{"type":"bold","offset":"4","length":0},false],
        "forward_origin":{"type":"future_origin","new_field":[1,false]},
        "new_field":{"opaque":" retained "}
    }});
    let parsed = parse(&payload);
    assert_eq!(parsed.metadata.event_type, "message");
    assert_eq!(parsed.raw.as_json(), Some(&payload));
    let PlatformData::Telegram(data) = parsed.data else {
        panic!()
    };
    let message = data.message.unwrap();
    assert!(message.from.is_none());
    assert_eq!(message.chat.unwrap().id, Some(-1001234567890));
    assert_eq!(message.text.as_deref(), Some(""));
    assert!(message.caption.is_none());
    let photo = message.photo.unwrap();
    assert_eq!(photo.len(), 1);
    assert_eq!(photo[0].file_id.as_deref(), Some(""));
    assert!(photo[0].width.is_none());
    assert_eq!(photo[0].height, Some(90));
    let entities = message.entities.unwrap();
    assert_eq!(entities.len(), 1);
    assert!(entities[0].offset.is_none());
    assert_eq!(entities[0].length, Some(0));
    let origin = message.forward_origin.unwrap();
    assert!(matches!(*origin, TelegramMessageOrigin::Unknown(_)));
    assert_eq!(
        serde_json::to_value(origin).unwrap(),
        payload["message"]["forward_origin"]
    );
}

#[test]
fn update_optionality_and_future_branches() {
    for (payload, kind) in [
        (json!({"update_id":1}), "update"),
        (
            json!({"update_id":1,"future_update":{"value":false}}),
            "future_update",
        ),
        (
            json!({"update_id":1,"message":{},"future_metadata":true}),
            "message",
        ),
    ] {
        let parsed = parse(&payload);
        assert_eq!(parsed.metadata.event_type, kind);
        assert_eq!(parsed.raw.as_json(), Some(&payload));
    }
    for payload in [
        json!({"update_id":-1}),
        json!({"update_id":"1"}),
        json!({"update_id":1,"message":{},"guest_message":{}}),
        json!({"update_id":1,"poll":{},"subscription":{}}),
    ] {
        assert!(
            receive_with(
                Provider::Telegram,
                Authentication::None,
                &headers(&[]),
                &serde_json::to_vec(&payload).unwrap()
            )
            .is_err()
        );
    }
}

#[test]
fn platform_data_is_the_typed_webhook_body_without_summary_or_update_wrapper() {
    let payload = json!({"update_id":123,"message":{
        "message_id":42,"date":1794268800,
        "chat":{"id":-1001234567890_i64,"type":"supergroup"},"text":"Hello"
    }});
    let parsed = parse(&payload);
    let data = serde_json::to_value(parsed.data).unwrap();
    assert_eq!(data, json!({"provider":"telegram","event":payload}));
    for field in ["chat_id", "message_id", "callback_query_id", "update"] {
        assert!(
            data["event"].get(field).is_none(),
            "unexpected synthetic field: {field}"
        );
    }
    assert!(serde_json::from_value::<TelegramEvent>(json!({"message":{}})).is_err());
}

#[test]
fn recursive_rich_text_replies_and_unknown_variants_round_trip() {
    let value = json!([" verbatim ",{"type":"bold","text":[{"type":"italic","text":"😀"}]},{"type":"future_text","payload":false}]);
    let text = TelegramRichText::read(&value).unwrap();
    assert_eq!(serde_json::to_value(&text).unwrap(), value);
    assert_eq!(
        serde_json::from_value::<TelegramRichText>(value).unwrap(),
        text
    );
    let mut reply = json!({"message_id":0,"date":1794268800,"text":"leaf"});
    for id in 1..12 {
        reply = json!({"message_id":id,"date":1794268800,"reply_to_message":reply});
    }
    let model = TelegramMessage::read(&reply).unwrap();
    assert_eq!(serde_json::to_value(model).unwrap(), reply);
    for value in [json!({"date":"0","message_id":1}), json!({"message_id":1})] {
        assert!(matches!(
            TelegramMaybeInaccessibleMessage::read(&value),
            Some(TelegramMaybeInaccessibleMessage::Unknown(_))
        ));
    }
}
