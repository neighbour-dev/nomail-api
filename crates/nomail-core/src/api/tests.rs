use super::*;
use serde_json::json;
use std::collections::HashSet;

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
            Operation::Health | Operation::Login | Operation::Refresh | Operation::Logout => {
                AuthRequirement::Public
            }
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
    assert_eq!(Operation::RawMessage.route().response, ResponseKind::Raw);
}

#[test]
fn static_routes_win_and_unknown_methods_are_not_authenticated_or_reinterpreted() {
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
fn query_dtos_normalize_and_validate_identically_for_both_transports() {
    let query = parse_query::<endpoints::ListMessages>(
        "status=%20read%20&level=HIGH&query=%20hello%20&pageSize=999",
    )
    .unwrap();
    let map = serde_json::to_value(&query).unwrap();
    assert_eq!(map["status"], "read");
    assert_eq!(map["level"], "high");
    assert_eq!(map["query"], "hello");
    assert_eq!(map["pageSize"], "999");
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
                .err()
                .unwrap()
                .status,
            400,
            "{query}"
        );
    }
}
