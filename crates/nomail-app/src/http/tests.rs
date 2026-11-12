use super::*;
use serde_json::json;
use std::collections::HashSet;

fn decode<E: Endpoint>(value: serde_json::Value) -> Result<E::Request, ApiFailure> {
    parse_json::<E>(
        &serde_json::to_vec(&value).unwrap(),
        Some("application/json"),
    )
}

#[test]
fn raw_response_wraps_exact_stored_strings_without_payload_decoding() {
    for (payload, content_type) in [
        (" {\n  \"title\": \"中文\"\n}\n", "application/json"),
        ("{not valid JSON", "application/json"),
        (" text\n 世界 \n", "text/plain"),
        (
            "text=%20one%20&text=two&empty=",
            "application/x-www-form-urlencoded",
        ),
        ("", ""),
        ("opaque stored data", "application/custom"),
    ] {
        let header = vec![
            json!({"name":"X-Unknown","value":" value ","encoding":null,"future":{"keep":[]}}),
        ];
        let response = MessageRawResponse::from(crate::message::RawContent {
            header: header.clone(),
            text: payload.into(),
            content_type: content_type.into(),
        });
        let value = serde_json::to_value(ApiResponse::success(response)).unwrap();
        assert_eq!(
            value,
            json!({"code":0,"msg":"success","data":{
                "header":header,"payload":payload,"contentType":content_type
            }})
        );
    }
}

#[test]
fn account_response_contains_only_id_and_username() {
    let response = AccountResponse::from(crate::auth::AccountInfo {
        id: 1,
        username: "admin".into(),
    });
    assert_eq!(
        serde_json::to_value(response).unwrap(),
        json!({"id":1,"username":"admin"})
    );
}

#[test]
fn auth_conversion_groups_registration_and_keeps_wire_defaults_at_the_boundary() {
    let login: crate::auth::Login = decode::<endpoints::Login>(json!({
        "username":"admin", "password":"password123", "deviceId":"phone", "platform":"ios", "deviceType":"phone",
        "deviceName":"Phone", "manufacturer":"Maker", "model":"Model", "osVersion":"OS", "appVersion":"App",
        "appBuild":"Build", "locale":"zh", "timeZone":"UTC"
    })).unwrap().into();
    assert_eq!(
        (login.username.as_str(), login.password.as_str()),
        ("admin", "password123")
    );
    let d = login.device;
    assert_eq!(
        [
            d.device_id,
            d.platform,
            d.device_type,
            d.device_name,
            d.manufacturer,
            d.model,
            d.os_version,
            d.app_version,
            d.app_build,
            d.locale,
            d.time_zone
        ],
        [
            "phone", "ios", "phone", "Phone", "Maker", "Model", "OS", "App", "Build", "zh", "UTC"
        ]
    );
    let update: crate::auth::UpdateAccount =
        decode::<endpoints::UpdateAccount>(json!({"currentPassword":"old"}))
            .unwrap()
            .into();
    assert_eq!(update.current_password, "old");
    assert!(update.username.is_empty() && update.new_password.is_empty());

    let now = chrono::DateTime::from_timestamp(1000, 0).unwrap();
    let device: crate::auth::Device = serde_json::from_value(json!({
        "id":1, "user_id":2, "device_id":"phone", "platform":"ios", "device_type":"phone",
        "created_at":now, "updated_at":now
    }))
    .unwrap();
    for session in [
        None,
        Some(crate::auth::Session {
            id: 3,
            device_id: 1,
            expires_at: now,
            logged_in_at: now,
            revoked_at: Some(now),
            deleted_at: None,
        }),
    ] {
        let expected_time = session
            .as_ref()
            .map(|_| json!(now))
            .unwrap_or(serde_json::Value::Null);
        let response = DeviceResponse::from(crate::auth::DeviceInfo {
            device: device.clone(),
            session,
            logged_in: false,
        });
        let value = serde_json::to_value(response).unwrap();
        for key in [
            "deviceName",
            "manufacturer",
            "model",
            "osVersion",
            "appVersion",
            "appBuild",
            "locale",
            "timeZone",
            "ipAddress",
            "userAgent",
        ] {
            assert_eq!(value[key], "", "{key}");
        }
        assert_eq!(value["expiresAt"], expected_time);
        assert_eq!(value["loggedInAt"], expected_time);
        assert_eq!(value["loggedIn"], false);
        assert!(value.get("userId").is_none());
    }
}

#[test]
fn business_previews_pages_and_counts_convert_to_the_existing_wire_shape() {
    let now = chrono::DateTime::from_timestamp(1000, 0).unwrap();
    let page: Page<MessageListItem> = crate::pagination::Page {
        items: vec![crate::message::MessagePreview {
            id: uuid::Uuid::from_u128(1),
            title: "Title".into(),
            summary: "Summary".into(),
            source: None,
            level: crate::webhook::MessageLevel::Normal,
            status: "unread".into(),
            pinned: false,
            webhook_config_id: 7,
            received_at: now,
            expires_at: None,
        }],
        total: 1,
        page: 2,
        page_size: 20,
    }
    .into();
    let value = serde_json::to_value(page).unwrap();
    assert_eq!(value["pageSize"], 20);
    assert_eq!(value["items"][0]["source"], "");
    assert_eq!(value["items"][0]["createdAt"], json!(now));
    assert_eq!(value["items"][0]["expireAt"], serde_json::Value::Null);
    assert_eq!(value["items"][0]["webhookConfigId"], 7);
    assert!(value["items"][0].get("receivedAt").is_none());
    assert_eq!(
        serde_json::to_value(CountResponse { count: 3 }).unwrap(),
        json!({"count":3})
    );
    assert_eq!(
        serde_json::to_value(AffectedResponse { affected: 4 }).unwrap(),
        json!({"affected":4})
    );
}

#[test]
fn message_updates_and_config_pagination_are_converted_before_use_cases() {
    use crate::message::UpdateMessage;
    for value in [
        json!({}),
        json!({"status":null, "pinned":null}),
        json!({"status":"READ"}),
        json!({"status":" read "}),
    ] {
        assert!(
            UpdateMessage::try_from(decode::<endpoints::UpdateMessage>(value).unwrap()).is_err()
        );
    }
    for value in [
        json!({"status":"read"}),
        json!({"status":"unread", "pinned":true}),
        json!({"status":null, "pinned":false}),
    ] {
        assert!(
            UpdateMessage::try_from(decode::<endpoints::UpdateMessage>(value).unwrap()).is_ok()
        );
    }
    for value in [json!({"status":false}), json!({"pinned":"true"})] {
        assert!(decode::<endpoints::UpdateMessage>(value).is_err());
    }
    let page = crate::webhook::config::ConfigPage::try_from(
        parse_query::<endpoints::ListConfigs>("page=0&pageSize=999").unwrap(),
    )
    .unwrap();
    assert_eq!((page.page(), page.size(), page.offset()), (1, 100, 0));
    assert!(
        crate::webhook::config::ConfigPage::try_from(
            parse_query::<endpoints::ListConfigs>("page=9223372036854775807&pageSize=100").unwrap()
        )
        .is_err()
    );
}

#[test]
fn route_inventory_is_unique_complete_and_resolves_with_shared_trailing_slash_policy() {
    assert_eq!(ROUTES.len(), Operation::ALL.len());
    let mut signatures = HashSet::new();
    let mut ids = HashSet::new();
    for (route, operation) in ROUTES.iter().zip(Operation::ALL) {
        assert_eq!(route.operation, *operation);
        assert_eq!(operation.route().path, route.path);
        assert!(signatures.insert((route.method.as_str(), route.path)));
        assert!(ids.insert(route.operation_id));
        let parameter = match route.path_kind {
            PathKind::ConfigId => "42",
            PathKind::MessageId => "0198c000-0000-7000-8000-000000000001",
            PathKind::DeviceId => "phone",
            _ => "abc123",
        };
        let path = if let Some(start) = route.path.find('{') {
            let end = route.path.find('}').unwrap();
            format!(
                "{}{}{}",
                &route.path[..start],
                parameter,
                &route.path[end + 1..]
            )
        } else {
            route.path.into()
        };
        for path in [path.clone(), format!("{path}/")] {
            let matched = resolve(route.method.as_str(), &path).unwrap();
            assert_eq!(matched.route.operation, *operation);
            if route.path_kind != PathKind::None {
                assert_eq!(matched.parameter().unwrap(), parameter);
            }
        }
    }
}

#[test]
fn authentication_and_non_json_contracts_are_declared_once() {
    for route in ROUTES {
        let expected = match route.operation {
            Operation::Health
            | Operation::SetupStatus
            | Operation::Setup
            | Operation::Login
            | Operation::Refresh
            | Operation::Logout => AuthRequirement::Public,
            Operation::ReceiveWebhook => AuthRequirement::Webhook,
            _ => AuthRequirement::Bearer,
        };
        assert_eq!(route.auth, expected);
    }
    assert_eq!(Operation::ReceiveWebhook.route().body, BodyKind::Raw);
    assert_eq!(
        Operation::ReceiveWebhook.route().response,
        ResponseKind::Webhook
    );
    assert_eq!(
        Operation::StreamMessages.route().response,
        ResponseKind::Sse
    );
    assert_eq!(Operation::RawMessage.route().response, ResponseKind::Json);
}

#[test]
fn static_routes_win_and_unknown_methods_are_not_authenticated_or_reinterpreted() {
    assert_eq!(
        resolve("GET", "/api/messages/cursor")
            .unwrap()
            .route
            .operation,
        Operation::CursorMessages
    );
    assert_eq!(
        resolve("POST", "/api/webhook/configs")
            .unwrap()
            .route
            .operation,
        Operation::CreateConfig
    );
    assert_eq!(
        resolve("GET", "/api/messages/unread-count")
            .unwrap()
            .route
            .operation,
        Operation::UnreadCount
    );
    assert_eq!(
        resolve("GET", "/api/messages/stream")
            .unwrap()
            .route
            .operation,
        Operation::StreamMessages
    );
    for (method, path) in [
        ("POST", "/api/health"),
        ("GET", "/api/messages/batch"),
        ("DELETE", "/api/messages/stream"),
        ("DELETE", "/api/messages/cursor"),
        ("GET", "/api//messages"),
        ("GET", "/api/unknown"),
        ("POST", "/api/push/devices"),
    ] {
        assert_eq!(
            resolve(method, path).err().unwrap().status,
            404,
            "{method} {path}"
        );
    }
}

#[test]
fn parameters_decode_once_and_url_generation_uses_the_receive_route() {
    let key = "a/b?中文%+";
    let url = webhook_url(key);
    let matched = resolve("POST", &url).unwrap();
    assert_eq!(matched.route.operation, Operation::ReceiveWebhook);
    assert_eq!(matched.parameter().unwrap(), key);
    assert_eq!(
        resolve("DELETE", "/api/auth/devices/a%252Fb")
            .unwrap()
            .parameter()
            .unwrap(),
        "a%2Fb"
    );
    for path in ["/api/auth/devices/%ZZ", "/api/auth/devices/%FF"] {
        assert_eq!(resolve("DELETE", path).err().unwrap().status, 400);
    }
    assert_eq!(
        resolve("GET", "/api/messages/not-a-uuid")
            .unwrap()
            .parameter()
            .err()
            .unwrap()
            .status,
        400
    );
    assert_eq!(
        resolve("GET", "/api/webhook/configs/0")
            .unwrap()
            .parameter()
            .err()
            .unwrap()
            .status,
        400
    );
}

#[test]
fn path_ids_are_parsed_into_business_types_without_a_string_roundtrip() {
    for id in ["1", "01", "%2B1", "%31", "9223372036854775807"] {
        let matched = resolve("GET", &format!("/api/webhook/configs/{id}")).unwrap();
        let expected = if id == "9223372036854775807" {
            i64::MAX
        } else {
            1
        };
        assert_eq!(matched.config_id().unwrap().get(), expected);
    }
    for id in ["0", "-1", "bad", "9223372036854775808", "1%20", "%252B1"] {
        let matched = resolve("GET", &format!("/api/webhook/configs/{id}")).unwrap();
        let error = matched.config_id().unwrap_err();
        assert_eq!(error.status, 400);
        assert_eq!(error.message, "invalid webhook config id");
    }
    let expected = uuid::Uuid::from_u128(0xabcd);
    for id in [
        expected.to_string(),
        expected.simple().to_string(),
        expected.to_string().to_uppercase(),
    ] {
        let matched = resolve("GET", &format!("/api/messages/{id}")).unwrap();
        assert_eq!(matched.message_id().unwrap(), expected);
    }
    let matched = resolve("GET", "/api/messages/not-a-uuid").unwrap();
    let error = matched.message_id().unwrap_err();
    assert_eq!(error.status, 400);
    assert_eq!(error.message, "invalid message id");
    assert_eq!(matched.config_id().unwrap_err().status, 500);
    let matched = resolve("GET", "/api/webhook/configs/1").unwrap();
    assert_eq!(matched.message_id().unwrap_err().status, 500);
}

#[test]
fn common_envelopes_and_json_limits_are_transport_independent() {
    assert_eq!(
        serde_json::to_value(ApiResponse::success(())).unwrap(),
        json!({"code":0,"msg":"success","data":null})
    );
    assert_eq!(
        serde_json::to_value(ApiFailure::unauthorized().response()).unwrap(),
        json!({"code":401,"msg":"unauthorized","data":null})
    );
    let body = br#"{"refreshToken":"test"}"#;
    assert!(
        parse_json::<endpoints::Refresh>(body, Some("Application/JSON; charset=utf-8")).is_ok()
    );
    assert!(parse_json::<endpoints::Refresh>(body, Some("application/vendor+json")).is_ok());
    assert_eq!(
        parse_json::<endpoints::Refresh>(body, Some("text/plain"))
            .err()
            .unwrap()
            .status,
        400
    );
    assert_eq!(
        parse_json::<endpoints::Refresh>(b"{", Some("application/json"))
            .err()
            .unwrap()
            .status,
        400
    );
    assert_eq!(
        parse_json::<endpoints::Refresh>(
            br#"{"refreshToken":"a","refreshToken":"b"}"#,
            Some("application/json")
        )
        .err()
        .unwrap()
        .status,
        400
    );
    assert_eq!(
        parse_json::<endpoints::CreateConfig>(
            br#"{"name":"missing provider"}"#,
            Some("application/json")
        )
        .err()
        .unwrap()
        .status,
        400
    );
    assert_eq!(
        parse_json::<endpoints::ReceiveWebhook>(b"[]", Some("application/json"))
            .err()
            .unwrap()
            .status,
        500
    );
    assert!(check_body_length(MAX_BODY_BYTES).is_ok());
    assert_eq!(
        check_body_length(MAX_BODY_BYTES + 1).unwrap_err().status,
        413
    );
}

#[test]
fn query_inputs_are_converted_to_types_without_a_string_roundtrip() {
    let request = parse_query::<endpoints::ListMessages>(
        "status=%20read%20&level=HIGH&query=%20hello%20&pageSize=999",
    )
    .unwrap();
    let map = serde_json::to_value(&request).unwrap();
    assert_eq!(map["status"], " read ");
    assert_eq!(map["level"], "HIGH");
    assert_eq!(map["query"], " hello ");
    assert_eq!(map["pageSize"], "999");
    let query = crate::message::MessageQuery::try_from(request).unwrap();
    assert_eq!(
        query.filters().status,
        Some(crate::message::ReadStatus::Read)
    );
    assert_eq!(
        query.filters().level,
        Some(crate::webhook::MessageLevel::High)
    );
    assert_eq!(query.filters().text.as_deref(), Some("hello"));
    assert_eq!(query.page_size(), 100);
    for query in [
        "level=success",
        "status=invalid",
        "from=bad",
        "webhookConfigId=0",
        "pinned=invalid",
        "page=1&page=2",
    ] {
        assert_eq!(
            parse_query::<endpoints::ListMessages>(query)
                .and_then(|request| crate::message::MessageQuery::try_from(request)
                    .map_err(ApiFailure::from))
                .err()
                .unwrap()
                .status,
            400,
            "{query}"
        );
    }
}

#[test]
fn query_conversion_preserves_legacy_defaults_aliases_times_and_large_offsets() {
    use crate::message::{MessageQuery, ReadStatus};
    use crate::webhook::MessageLevel;
    let raw: ListMessagesQuery = serde_json::from_value(json!({
        "page":"0", "pageSize":"999", "query":" %hello_ ", "status":" read ",
        "level":"HIGH", "pinned":"T", "webhookConfigId":"2",
        "from":"2026-01-01T01:00:00+01:00", "to":"2026-01-02T00:00:00Z"
    }))
    .unwrap();
    let query = MessageQuery::try_from(raw).unwrap();
    assert_eq!(query.page(), 1);
    assert_eq!(query.page_size(), 100);
    assert_eq!(query.filters().text.as_deref(), Some("%hello_"));
    assert_eq!(query.filters().status, Some(ReadStatus::Read));
    assert_eq!(query.filters().level, Some(MessageLevel::High));
    assert_eq!(query.filters().webhook_config_id, Some(2));
    assert_eq!(
        query.filters().from.unwrap().to_rfc3339(),
        "2026-01-01T00:00:00+00:00"
    );
    for (input, expected) in [
        ("1", true),
        ("t", true),
        ("T", true),
        ("TRUE", true),
        ("true", true),
        ("True", true),
        ("0", false),
        ("f", false),
        ("F", false),
        ("FALSE", false),
        ("false", false),
        ("False", false),
    ] {
        let raw = parse_query::<endpoints::ListMessages>(&format!("pinned={input}")).unwrap();
        assert_eq!(
            MessageQuery::try_from(raw).unwrap().filters().pinned,
            Some(expected)
        );
    }
    for input in [
        "",
        "page=0&pageSize=0",
        "page=-1&pageSize=-1",
        "page=bad&pageSize=bad",
        "page=9223372036854775808&pageSize=9223372036854775808",
    ] {
        let query =
            MessageQuery::try_from(parse_query::<endpoints::ListMessages>(input).unwrap()).unwrap();
        assert_eq!(
            (query.page(), query.page_size(), query.offset()),
            (1, 20, 0)
        );
    }
    let query = MessageQuery::try_from(
        parse_query::<endpoints::ListMessages>("page=9007199254740994&pageSize=1").unwrap(),
    )
    .unwrap();
    assert_eq!(query.offset(), 9_007_199_254_740_993);
    for input in [
        "level=critical",
        "status=READ",
        "pinned=%20true%20",
        "webhookConfigId=-1",
        "from=%20",
        "from=2026-01-02T00%3A00%3A00Z&to=2026-01-01T00%3A00%3A00Z",
        "page=9223372036854775807&pageSize=100",
    ] {
        assert!(
            MessageQuery::try_from(parse_query::<endpoints::ListMessages>(input).unwrap()).is_err(),
            "{input}"
        );
    }
}

#[test]
fn configuration_inputs_preserve_required_fields_empty_patch_and_strict_null_behavior() {
    use crate::webhook::config::{CreateConfig, UpdateConfig};
    let create = |value| {
        decode::<endpoints::CreateConfig>(value)
            .and_then(|request| CreateConfig::try_from(request).map_err(ApiFailure::from))
    };
    let update = |value| {
        decode::<endpoints::UpdateConfig>(value)
            .and_then(|request| UpdateConfig::try_from(request).map_err(ApiFailure::from))
    };
    let platform = json!({"provider":"github","credentials":{"secret":"test-secret"}});
    assert!(create(json!({"name":"Name", "platform":platform})).is_ok());
    assert!(create(json!({"name":"Name", "platform":platform, "futureField":true})).is_err());
    for input in [
        json!({}),
        json!({"name":"Name"}),
        json!({"provider":"generic"}),
        json!({"name":"", "provider":"generic"}),
        json!({"name":"Name", "provider":""}),
        json!({"name":"Name", "provider":"stripe"}),
    ] {
        assert_eq!(create(input).err().unwrap().status, 400);
    }
    assert!(update(json!({})).unwrap().is_empty());
    assert!(update(json!({"name":""})).is_err());
    assert!(!update(json!({"description":""})).unwrap().is_empty());
    assert_eq!(update(json!({"provider":" "})).err().unwrap().status, 400);
    for field in ["name", "platform", "description"] {
        for invalid in [serde_json::Value::Null, json!(42)] {
            let mut input = json!({"name":"Name", "platform":platform});
            input[field] = invalid.clone();
            assert_eq!(create(input).err().unwrap().status, 400, "create {field}");
            let mut input = json!({});
            input[field] = invalid;
            assert_eq!(update(input).err().unwrap().status, 400, "update {field}");
        }
    }
    for (field, limit) in [("name", 100), ("description", 500)] {
        let mut input = json!({"name":"Name", "platform":platform});
        input[field] = json!("中".repeat(limit));
        assert!(create(input.clone()).is_ok(), "{field} limit");
        input[field] = json!("中".repeat(limit + 1));
        assert_eq!(
            create(input).err().unwrap().status,
            400,
            "create {field} limit"
        );
        let mut input = json!({});
        input[field] = json!("中".repeat(limit + 1));
        assert_eq!(
            update(input).err().unwrap().status,
            400,
            "update {field} limit"
        );
    }
    for body in [
        br#"{"name":"a","name":"b","platform":{"provider":"github","credentials":{"secret":"one"}}}"#.as_slice(),
        br#"{"name":"a","platform":{"provider":"github","provider":"generic","credentials":{"secret":"one"}}}"#.as_slice(),
        br#"{"name":"a","platform":{"provider":"github","credentials":{"secret":"one","secret":"two"}}}"#.as_slice(),
        br#"[]"#.as_slice(),
        br#"["Name","generic"]"#.as_slice(),
        br#"["Name","generic","","",""]"#.as_slice(),
    ] {
        assert_eq!(
            parse_json::<endpoints::CreateConfig>(body, Some("application/json"))
                .err()
                .unwrap()
                .status,
            400
        );
    }
    assert_eq!(
        parse_json::<endpoints::UpdateConfig>(
            br#"{"secret":"a","secret":"b"}"#,
            Some("application/json")
        )
        .err()
        .unwrap()
        .status,
        400
    );
}

#[test]
fn batch_conversion_preserves_limits_uuid_errors_action_errors_and_deduplication() {
    use crate::message::{BatchAction, BatchMessages};
    let id = uuid::Uuid::from_u128(1).to_string();
    let convert = |value| {
        decode::<endpoints::BatchMessages>(value)
            .and_then(|request| BatchMessages::try_from(request).map_err(ApiFailure::from))
    };
    for (action, expected) in [
        ("mark_read", BatchAction::MarkRead),
        ("mark_unread", BatchAction::MarkUnread),
        ("pin", BatchAction::Pin),
        ("unpin", BatchAction::Unpin),
        ("delete", BatchAction::Delete),
    ] {
        let command = convert(json!({"ids":[id,id],"action":action})).unwrap();
        assert_eq!(command.action(), expected);
        assert_eq!(command.ids(), &[uuid::Uuid::from_u128(1)]);
    }
    assert_eq!(
        convert(json!({"ids":vec![id.clone();100],"action":"pin"}))
            .unwrap()
            .ids()
            .len(),
        1
    );
    for (input, message) in [
        (
            json!({"ids":[],"action":"invalid"}),
            "ids must contain 1 to 100 items",
        ),
        (
            json!({"ids":vec!["invalid";101],"action":"invalid"}),
            "ids must contain 1 to 100 items",
        ),
        (
            json!({"ids":[id,"invalid"],"action":"invalid"}),
            "invalid message id",
        ),
        (
            json!({"ids":[id],"action":"invalid"}),
            "unsupported batch action",
        ),
        (
            json!({"ids":[id],"action":"PIN"}),
            "unsupported batch action",
        ),
    ] {
        let error = convert(input).err().unwrap();
        assert_eq!(error.status, 400);
        assert_eq!(error.message, message);
    }
    for input in [
        json!({"ids":null,"action":"pin"}),
        json!({"ids":[id],"action":null}),
        json!({"ids":[42],"action":"pin"}),
    ] {
        assert_eq!(convert(input).err().unwrap().status, 400);
    }
}
