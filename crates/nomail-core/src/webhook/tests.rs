//! Cross-platform API contracts. Platform cases and fixtures live in platforms/*/tests.rs.
use super::{test_support::*, *};
use serde_json::{Value, json};

#[test]
fn every_platform_binds_allowlisted_headers_to_native_payload_fields() {
    let cases = [
        (
            Provider::Generic,
            vec![("X-NoMail-Idempotency-Key", "generic-1")],
            json!({"id":123,"title":" verbatim ","content":"","eventType":"created"}),
            json!({"idempotency_key":"generic-1"}),
        ),
        (
            Provider::Github,
            vec![
                ("X-GitHub-Event", "push"),
                ("X-GitHub-Delivery", "github-1"),
                ("X-GitHub-Hook-Id", "000123"),
            ],
            json!({"ref":"refs/heads/main","before":"aaa","after":"bbb","forced":false,"pusher":{"name":" Actor "},"commits":[{"id":"bbb","message":" commit\n"}]}),
            json!({"event":"push","delivery":"github-1","hook_id":"000123"}),
        ),
        (
            Provider::Gitea,
            vec![
                ("X-Gitea-Event", "wiki"),
                ("X-Gitea-Event-Type", "wiki"),
                ("X-Gitea-Delivery", "gitea-1"),
                ("X-Gitea-Hook-Installation-Target-Type", "repository"),
            ],
            json!({"action":"edited","page":"Home","comment":" update\n"}),
            json!({"event":"wiki","event_type":"wiki","delivery":"gitea-1","installation_target_type":"repository"}),
        ),
        (
            Provider::AppStoreConnect,
            vec![],
            json!({"data":{"type":"buildUploadStateUpdated","id":"apple-1","attributes":{"oldState":"PROCESSING","newState":"COMPLETE"}}}),
            json!({}),
        ),
        (
            Provider::Gitlab,
            vec![
                ("X-Gitlab-Event", "Issue Hook"),
                ("X-Gitlab-Instance", "https://example.invalid"),
                ("webhook-id", "gitlab-1"),
            ],
            json!({"object_kind":"issue","project":{"path_with_namespace":"example/repo"},"object_attributes":{"action":"open","title":" issue "}}),
            json!({"event":"Issue Hook","instance":"https://example.invalid","webhook_id":"gitlab-1"}),
        ),
        (
            Provider::Stripe,
            vec![],
            json!({"id":"evt_1","type":"payment_intent.succeeded","object":"event","created":1794268800,"data":{"object":{"id":"pi_1","object":"payment_intent","amount":100}}}),
            json!({}),
        ),
        (
            Provider::Slack,
            vec![
                ("X-Slack-Retry-Num", "1"),
                ("X-Slack-Retry-Reason", "http_timeout"),
            ],
            json!({"type":"event_callback","event_id":"Ev1","team_id":"T1","event":{"type":"message","text":" text\n","subtype":"bot_message"}}),
            json!({"retry_num":"1","retry_reason":"http_timeout"}),
        ),
        (
            Provider::Feishu,
            vec![],
            json!({"schema":"2.0","header":{"event_id":"feishu-1","event_type":"im.message.receive_v1","app_id":"app-1"},"event":{"sender":{"sender_id":{"open_id":"ou_1"},"sender_type":"user"},"message":{"content":"{\"text\":\"hello\"}","message_type":"text"}}}),
            json!({}),
        ),
        (
            Provider::Telegram,
            vec![],
            json!({"update_id":1,"message":{"message_id":0,"date":1794268800,"chat":{"id":-1001234567890_i64,"type":"supergroup"},"text":" hi "}}),
            json!({}),
        ),
        (
            Provider::Shopify,
            vec![
                ("X-Shopify-Topic", "products/update"),
                ("X-Shopify-Shop-Domain", "example.myshopify.com"),
                ("X-Shopify-Api-Version", "2026-10"),
                ("X-Shopify-Webhook-Id", "shopify-1"),
                ("X-Shopify-Event-Id", "operation-1"),
            ],
            json!({"id":123,"title":" product "}),
            json!({"topic":"products/update","shop_domain":"example.myshopify.com","api_version":"2026-10","webhook_id":"shopify-1","event_id":"operation-1"}),
        ),
    ];
    for (provider, mut input_headers, payload, expected_header) in cases {
        input_headers.extend([
            ("Authorization", "Bearer private-test-credential"),
            ("X-Telegram-Bot-Api-Secret-Token", "private-test-credential"),
            ("X-Gitlab-Token", "private-test-credential"),
            ("X-Hub-Signature-256", "private-test-signature"),
            ("X-Gitea-Signature", "private-test-signature"),
            ("Stripe-Signature", "private-test-signature"),
            ("X-Slack-Signature", "private-test-signature"),
        ]);
        let parsed = event(
            receive_with(
                provider,
                Authentication::None,
                &headers(&input_headers),
                &serde_json::to_vec(&payload).unwrap(),
            )
            .unwrap(),
        );
        assert_eq!(parsed.raw.as_json(), Some(&payload));
        let stored = RawWebhook::from_stored(
            parsed.raw.header.clone(),
            &parsed.raw.stored_payload(),
            parsed.raw.payload_content_type(),
        )
        .unwrap();
        assert_eq!(stored, parsed.raw);
        assert_eq!(
            parse_raw(provider, &stored).ok(),
            parsed.data,
            "{provider:?}: raw-only read projection differs from ingestion"
        );
        let data = serde_json::to_value(&parsed.data).unwrap();
        let envelope = &data["event"];
        assert_eq!(envelope.as_object().unwrap().len(), 2);
        assert_eq!(envelope["header"], expected_header, "{provider:?}");
        assert_eq!(
            envelope["payload"], payload,
            "{provider:?}: payload changed supplied fields or invented defaults"
        );
        assert!(!data.to_string().contains("private-test-"));
        assert_eq!(
            serde_json::from_value::<PlatformEvent>(serde_json::to_value(&parsed).unwrap())
                .unwrap(),
            parsed
        );
    }
}

#[test]
fn native_payloads_preserve_missing_null_and_supplied_values_exactly() {
    let cases = [
        (
            Provider::Generic,
            vec![],
            json!({"title":null,"content":"","level":"normal","id":0}),
        ),
        (
            Provider::Github,
            vec![("X-GitHub-Event", "star")],
            json!({"action":"deleted","starred_at":null}),
        ),
        (
            Provider::Github,
            vec![("X-GitHub-Event", "pull_request")],
            json!({"action":"opened","pull_request":{"body":null,"title":"","assignees":[],"head":{"ref":"feature","repo":null},"base":null,"mergeable":null,"merged":false,"additions":0}}),
        ),
        (
            Provider::Gitea,
            vec![("X-Gitea-Event", "issues")],
            json!({"issue":{"id":0,"title":"","milestone":null,"assignee":null,"labels":[],"is_locked":false},"changes":{"removed_labels":null}}),
        ),
        (
            Provider::AppStoreConnect,
            vec![],
            json!({"data":{"type":"alternativeDistributionTerritoryAvailabilityUpdated","id":null,"version":null,"attributes":{"timestamp":null,"available":false,"territories":[],"appId":""},"relationships":{"instance":{"data":null}}}}),
        ),
        (
            Provider::AppStoreConnect,
            vec![],
            json!({"data":{"type":" futureEvent ","attributes":{"oldValue":null,"newValue":" value "}}}),
        ),
        (
            Provider::Gitlab,
            vec![("X-Gitlab-Event", "Issue Hook")],
            json!({"object_kind":"issue","project":null,"object_attributes":{"milestone_id":null,"time_estimate":0,"description":""},"commits":[null,0,false,""]}),
        ),
        (
            Provider::Stripe,
            vec![],
            json!({"id":"evt_presence","object":"event","type":"customer.updated","api_version":null,"request":null,"livemode":false,"pending_webhooks":0,"data":{"object":{"id":"cus_1","email":null},"previous_attributes":{"email":null,"metadata":{"key":null}}}}),
        ),
        (
            Provider::Slack,
            vec![],
            json!({"type":"event_callback","team_id":null,"event":{"type":"message","text":"","user":null},"view":{"state":{"values":{"input":{"value":null,"selected_conversations":[]}}}}}),
        ),
        (
            Provider::Feishu,
            vec![],
            json!({"schema":"2.0","header":{"event_type":"im.message.receive_v1","tenant_key":null},"event":{"sender":{"sender_id":{"open_id":"ou_1","union_id":null}},"message":{"message_id":"m1","parent_id":null,"content":"","message_type":"text"}}}),
        ),
        (
            Provider::Telegram,
            vec![],
            json!({"update_id":0,"message":{"message_id":0,"date":1,"chat":{"id":0,"type":"private"},"from":null,"text":"","entities":[],"reply_to_message":null}}),
        ),
        (
            Provider::Shopify,
            vec![
                ("X-Shopify-Topic", "products/update"),
                ("X-Shopify-Shop-Domain", "example.myshopify.com"),
            ],
            json!({"id":0,"title":"","name":null}),
        ),
    ];
    for (provider, input_headers, payload) in cases {
        let parsed = event(
            receive_with(
                provider,
                Authentication::None,
                &headers(&input_headers),
                &serde_json::to_vec(&payload).unwrap(),
            )
            .unwrap(),
        );
        let data = serde_json::to_value(&parsed.data).unwrap();
        assert_eq!(
            data["event"]["payload"], payload,
            "{provider:?}: presence/type/value changed"
        );
        let restored: PlatformEvent =
            serde_json::from_value(serde_json::to_value(&parsed).unwrap()).unwrap();
        assert_eq!(
            restored, parsed,
            "{provider:?}: stored snapshot lost presence state"
        );
    }
    // Dynamic JSON values, including nested nulls and null array elements, are
    // values too; they are not recursively pruned by the Option serializer.
    for value in [
        Value::Null,
        json!([]),
        json!({}),
        json!(""),
        json!(false),
        json!(0),
        json!([null, false, 0, "", []]),
        json!({"null":null}),
    ] {
        let payload = json!({"id":value});
        let parsed = event(
            receive_with(
                Provider::Generic,
                Authentication::None,
                &headers(&[]),
                &serde_json::to_vec(&payload).unwrap(),
            )
            .unwrap(),
        );
        assert_eq!(
            serde_json::to_value(parsed.data).unwrap()["event"]["payload"],
            payload
        );
    }
}

#[test]
fn github_and_gitea_do_not_serialize_synthetic_payload_groups() {
    for (provider, prefix) in [
        (Provider::Github, "X-GitHub-Event"),
        (Provider::Gitea, "X-Gitea-Event"),
    ] {
        for (kind, body) in [
            (
                "push",
                json!({"ref":"refs/heads/main","before":"a","after":"b","commits":[]}),
            ),
            ("create", json!({"ref":"main","ref_type":"branch"})),
            (
                "status",
                json!({"id":1,"state":"success","sha":"b","context":"ci"}),
            ),
        ] {
            let parsed = event(
                receive_with(
                    provider,
                    Authentication::None,
                    &headers(&[(prefix, kind)]),
                    &serde_json::to_vec(&body).unwrap(),
                )
                .unwrap(),
            );
            let data = serde_json::to_value(parsed.data).unwrap();
            for field in [
                "push",
                "git_ref",
                "commit_status",
                "wiki",
                "event_type",
                "installation_target_type",
            ] {
                assert!(data["event"]["payload"].get(field).is_none());
            }
            for (field, value) in body.as_object().unwrap() {
                assert_eq!(&data["event"]["payload"][field], value);
            }
        }
    }
}

#[test]
fn native_payload_extraction_does_not_coerce_scalars_or_copy_verification_tokens() {
    for (provider, event_header) in [
        (Provider::Github, "x-github-event"),
        (Provider::Gitea, "x-gitea-event"),
    ] {
        let body = json!({"ref":123,"repository":{"name":" name ","description":""},"sender":{"login":false}});
        let parsed = event(
            receive_with(
                provider,
                Authentication::None,
                &headers(&[(event_header, "push")]),
                &serde_json::to_vec(&body).unwrap(),
            )
            .unwrap(),
        );
        let model = serde_json::to_value(&parsed.data).unwrap();
        assert!(model["event"]["payload"]["ref"].is_null());
        assert!(model["event"]["payload"]["sender"]["login"].is_null());
        assert_eq!(model["event"]["payload"]["repository"]["name"], " name ");
        assert_eq!(model["event"]["payload"]["repository"]["description"], "");
        assert_eq!(parsed.raw.as_json(), Some(&body));
    }
    let body = json!({"schema":"2.0","header":{"token":"verification-secret","event_id":"feishu-1","event_type":"im.message.receive_v1"},"event":{"message":{"message_type":"text","content":"{\"text\":\"hello\"}"}}});
    let parsed = event(
        receive_with(
            Provider::Feishu,
            Authentication::None,
            &headers(&[]),
            &serde_json::to_vec(&body).unwrap(),
        )
        .unwrap(),
    );
    let model = serde_json::to_value(&parsed.data).unwrap();
    assert!(!model.to_string().contains("verification-secret"));
    assert!(
        model["event"]["payload"]["event"]["message"]
            .get("content_json")
            .is_none()
    );
    // Raw retains its existing complete-decoded-content contract; the safe
    // typed model is not a promise to redact arbitrary upstream raw content.
    assert_eq!(parsed.raw.as_json(), Some(&body));
}

#[test]
fn configuration_is_explicit_and_headers_are_case_insensitive() {
    assert_eq!(" GitHub ".parse::<Provider>().unwrap(), Provider::Github);
    assert_eq!(" Gitea ".parse::<Provider>().unwrap(), Provider::Gitea);
    assert_eq!(
        " AppStoreConnect ".parse::<Provider>().unwrap(),
        Provider::AppStoreConnect
    );
    assert!("unknown".parse::<Provider>().is_err());
    let h = headers(&[("X-Repeated", "one"), ("x-repeated", "two")]);
    assert_eq!(h.values("X-REPEATED"), ["one", "two"]);
    assert!(matches!(
        receive_with(
            Provider::Github,
            Authentication::TelegramSecretToken("secret"),
            &h,
            b"{}"
        ),
        Err(WebhookError::InvalidConfiguration(_))
    ));
    assert!(matches!(
        receive_with(Provider::Github, Authentication::HmacSha256(""), &h, b"{}"),
        Err(WebhookError::InvalidConfiguration(_))
    ));
    let config = WebhookConfig::new(Provider::Generic, Authentication::None, -1);
    assert!(matches!(
        receive(
            &config,
            &WebhookRequest {
                headers: &h,
                body: b"{}"
            }
        ),
        Err(WebhookError::InvalidConfiguration(_))
    ));
}

#[test]
fn undecodable_json_is_rejected_but_unmodeled_json_is_retained() {
    for provider in [
        Provider::Gitea,
        Provider::AppStoreConnect,
        Provider::Gitlab,
        Provider::Stripe,
        Provider::Slack,
        Provider::Feishu,
        Provider::Telegram,
        Provider::Shopify,
    ] {
        for body in [b"invalid".as_slice(), b"{"] {
            assert!(receive_with(provider, Authentication::None, &Vec::new(), body).is_err());
        }
        for body in [b"null".as_slice(), b"[]", b"{}"] {
            let parsed =
                event(receive_with(provider, Authentication::None, &Vec::new(), body).unwrap());
            assert_eq!(parsed.provider(), provider);
            assert_eq!(
                parsed.raw.as_json().unwrap(),
                &serde_json::from_slice::<Value>(body).unwrap()
            );
            assert!(!parsed.to_message_content().title.is_empty());
            assert!(!parsed.to_message_content().content.is_empty());
            assert!(parsed.metadata.deduplication_key.is_none());
        }
    }
}

#[test]
fn normalized_levels_are_stable() {
    for (label, expected) in [
        (" LOW ", MessageLevel::Low),
        ("normal", MessageLevel::Normal),
        ("High", MessageLevel::High),
        (" URGENT ", MessageLevel::Urgent),
        ("error", MessageLevel::Normal),
        ("warning", MessageLevel::Normal),
        ("critical", MessageLevel::Normal),
        ("success", MessageLevel::Normal),
        ("failed", MessageLevel::Normal),
        ("info", MessageLevel::Normal),
    ] {
        assert_eq!(MessageLevel::from_label(label), expected);
    }
    for label in ["low", "normal", "high", "urgent"] {
        let level: MessageLevel = label.parse().unwrap();
        assert_eq!(
            serde_json::to_string(&level).unwrap(),
            format!("\"{label}\"")
        );
        assert_eq!(level.as_str(), label);
    }
    assert!("critical".parse::<MessageLevel>().is_err());
}

#[test]
fn readable_content_has_explicit_format_and_is_serializable() {
    let contents = [
        EventContent::Json(serde_json::json!({"text":"你好","unknown":[1,2]})),
        EventContent::Text(" untouched text\n".into()),
        EventContent::Form(vec![
            ("command".into(), "/notify".into()),
            ("tag".into(), "a".into()),
            ("tag".into(), "b".into()),
        ]),
    ];
    for content in contents {
        let encoded = serde_json::to_value(&content).unwrap();
        let decoded: EventContent = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded, content);
        assert_eq!(decoded.to_readable_string(), content.to_readable_string());
    }
    let json = EventContent::Json(serde_json::json!({"text":"你好"}));
    assert!(json.to_readable_string().contains("你好"));
    assert_eq!(json.media_type(), "application/json");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&json.to_readable_string()).unwrap(),
        *json.as_json().unwrap()
    );
}

#[test]
fn event_envelope_roundtrips_without_losing_unknown_content() {
    let h = headers(&[
        ("X-GitHub-Event", "issues"),
        ("X-GitHub-Delivery", "delivery-1"),
    ]);
    let parsed = event(
        receive_with(
            Provider::Github,
            Authentication::None,
            &h,
            br#"{"action":"opened","issue":{"title":"Issue"},"unknown":{"keep":true}}"#,
        )
        .unwrap(),
    );
    let before = parsed.clone();
    let message = parsed.to_message_content();
    assert_eq!(parsed, before);
    let decoded: PlatformEvent =
        serde_json::from_str(&serde_json::to_string(&parsed).unwrap()).unwrap();
    assert_eq!(decoded, parsed);
    assert_eq!(decoded.to_message_content(), message);
    assert_eq!(
        decoded.metadata.deduplication_key.as_ref().unwrap().value,
        "delivery-1"
    );
    assert_eq!(decoded.raw.as_json().unwrap()["unknown"]["keep"], true);
}
