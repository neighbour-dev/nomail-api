//! OpenAPI 3.1 is generated from ROUTES and the actual Rust DTO schemas. This
//! module is feature-gated and is not linked into normal server/WASM builds.
use super::*;
use schemars::{JsonSchema, generate::SchemaSettings};
use serde_json::{Map, Value, json};

pub fn request_schema<T: JsonSchema>() -> Value {
    serde_json::to_value(
        SchemaSettings::draft2020_12()
            .for_deserialize()
            .into_generator()
            .into_root_schema_for::<T>(),
    )
    .expect("schema serializes")
}
pub fn response_schema<T: JsonSchema>() -> Value {
    serde_json::to_value(
        SchemaSettings::draft2020_12()
            .for_serialize()
            .into_generator()
            .into_root_schema_for::<T>(),
    )
    .expect("schema serializes")
}
fn rewrite_refs(value: &mut Value) {
    match value {
        Value::Object(object) => {
            if let Some(Value::String(reference)) = object.get_mut("$ref")
                && let Some(name) = reference.strip_prefix("#/$defs/")
            {
                *reference = format!("#/components/schemas/{name}");
            }
            for value in object.values_mut() {
                rewrite_refs(value);
            }
        }
        Value::Array(values) => {
            for value in values {
                rewrite_refs(value);
            }
        }
        _ => (),
    }
}
fn register(mut root: Value, schemas: &mut Map<String, Value>) -> Value {
    if let Some(object) = root.as_object_mut() {
        object.remove("$schema");
        if let Some(Value::Object(definitions)) = object.remove("$defs") {
            for (name, mut schema) in definitions {
                rewrite_refs(&mut schema);
                if let Some(old) = schemas.get(&name) {
                    assert_eq!(old, &schema, "inconsistent schema name: {name}");
                }
                schemas.insert(name, schema);
            }
        }
    }
    rewrite_refs(&mut root);
    root
}

pub fn document() -> Value {
    let mut schemas = Map::new();
    let mut paths = Map::new();
    let error_schema = register(response_schema::<ApiResponse<()>>(), &mut schemas);
    for route in ROUTES {
        let response = register((route.response_schema)(), &mut schemas);
        let content = match route.response {
            ResponseKind::Json => json!({"application/json":{"schema":response}}),
            ResponseKind::Sse => {
                json!({"text/event-stream":{"schema":{"type":"string"},"example":"event: ready\ndata: {}\n\n"}})
            }
            ResponseKind::Webhook => {
                json!({"application/json":{"schema":{"oneOf":[response,{"type":"object","required":["challenge"],"properties":{"challenge":{"type":"string"}}}]}},"text/plain":{"schema":{"type":"string"},"example":"pong"}})
            }
            ResponseKind::Raw => {
                json!({"application/json":{"schema":true},"text/plain":{"schema":{"type":"string"}},"application/octet-stream":{"schema":{"type":"string","format":"binary"}}})
            }
        };
        let mut responses = Map::new();
        responses.insert("200".into(),json!({"description":"成功；SSE、原文、Webhook 控制响应不使用 NoMail JSON 包装","content":content}));
        for status in [400, 401, 404, 409, 413, 500] {
            responses.insert(status.to_string(),json!({"description":format!("HTTP {status}"),"content":{"application/json":{"schema":error_schema}}}));
        }
        if route.response == ResponseKind::Sse {
            responses.insert("429".into(),json!({"description":"容量或请求限流（由部署环境执行）","headers":{"Retry-After":{"schema":{"type":"string"}}},"content":{"application/json":{"schema":error_schema}}}));
        }
        let mut operation = json!({"operationId":route.operation_id,"summary":route.summary,"tags":[route.tag],"responses":responses,
            "security":if route.auth==AuthRequirement::Bearer{json!([{"BearerAuth":[]}])}else{json!([])},
            "x-body-limit-bytes":MAX_BODY_BYTES});
        if route.response == ResponseKind::Sse {
            operation["x-sse-events"] = register(response_schema::<sse::SseEvent>(), &mut schemas);
        }
        let mut parameters = Vec::new();
        if route.path_kind != PathKind::None {
            let name = route
                .path
                .split('{')
                .nth(1)
                .unwrap()
                .split('}')
                .next()
                .unwrap();
            let schema = match route.path_kind {
                PathKind::ConfigId => json!({"type":"integer","format":"int64","minimum":1}),
                PathKind::MessageId => json!({"type":"string","format":"uuid"}),
                PathKind::DeviceId => json!({"type":"string","maxLength":128}),
                _ => json!({"type":"string"}),
            };
            parameters.push(json!({"name":name,"in":"path","required":true,"schema":schema}));
        }
        if route.has_query {
            let query = register((route.query_schema)(), &mut schemas);
            if let Some(properties) = query["properties"].as_object() {
                for (name, schema) in properties {
                    parameters
                        .push(json!({"name":name,"in":"query","required":false,"schema":schema}));
                }
            }
        }
        if !parameters.is_empty() {
            operation["parameters"] = json!(parameters);
        }
        match route.body {
            BodyKind::Json => {
                let schema = register((route.request_schema)(), &mut schemas);
                operation["requestBody"] =
                    json!({"required":true,"content":{"application/json":{"schema":schema}}});
            }
            BodyKind::Raw => {
                operation["requestBody"] = json!({"required":true,"content":{"application/json":{"schema":true},"text/plain":{"schema":{"type":"string"}},"application/x-www-form-urlencoded":{"schema":{"type":"string"}},"application/octet-stream":{"schema":{"type":"string","format":"binary"}}}});
                operation["description"] = json!(
                    "保留原始字节验签；认证方式由接收配置决定，不要求 Bearer JWT。平台验证/Ping 直接返回协议响应，普通消息入库后返回 NoMail ACK。"
                );
            }
            BodyKind::None => (),
        }
        let path = paths.entry(route.path).or_insert_with(|| json!({}));
        path[route.method.as_str().to_ascii_lowercase()] = operation;
    }
    json!({"openapi":"3.1.0","info":{"title":"NoMail API","version":"1.0.0","description":"由 nomail-app HTTP 契约与 Rust DTO 自动生成，请勿手动修改。成功 code=0，错误 code=HTTP 状态，错误 data=null；未知路径和不支持的方法均返回 404。"},
        "servers":[{"url":"/"}],"paths":paths,"components":{"securitySchemes":{"BearerAuth":{"type":"http","scheme":"bearer","bearerFormat":"JWT"}},"schemas":schemas}})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_openapi_matches_routes_models_and_checked_in_document() {
        let document = document();
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/swagger.json");
        let checked: Value = serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
        assert_eq!(
            document, checked,
            "regenerate with cargo run -p nomail-app --features openapi --example export-openapi -- docs/swagger.json"
        );
        for route in ROUTES {
            let operation =
                &document["paths"][route.path][route.method.as_str().to_ascii_lowercase()];
            assert_eq!(operation["operationId"], route.operation_id);
            assert_eq!(
                operation["security"],
                if route.auth == AuthRequirement::Bearer {
                    json!([{"BearerAuth":[]}])
                } else {
                    json!([])
                }
            );
        }
        let create_schema = &document["paths"]["/api/webhook/configs"]["post"]["requestBody"]["content"]
            ["application/json"]["schema"];
        let update_schema = &document["paths"]["/api/webhook/configs/{id}"]["patch"]["requestBody"]
            ["content"]["application/json"]["schema"];
        assert_eq!(create_schema["required"], json!(["name", "platform"]));
        assert!(create_schema["properties"]["name"].get("default").is_none());
        assert!(
            create_schema["properties"]["platform"]
                .get("default")
                .is_none()
        );
        assert!(update_schema.get("required").is_none());
        for field in ["name", "description", "platform"] {
            assert!(update_schema["properties"][field].get("default").is_none());
            assert!(update_schema["properties"][field].get("anyOf").is_none());
        }
        assert_eq!(create_schema["additionalProperties"], false);
        assert_eq!(update_schema["additionalProperties"], false);
        assert!(create_schema["properties"].get("source").is_none());
        let platform = &document["components"]["schemas"]["PlatformConfig"];
        assert_eq!(platform["oneOf"].as_array().unwrap().len(), 10);
        fn references(value: &Value, root: &Value) {
            match value {
                Value::Object(object) => {
                    if let Some(Value::String(reference)) = object.get("$ref") {
                        assert!(
                            root.pointer(
                                reference.strip_prefix('#').expect("local schema reference")
                            )
                            .is_some(),
                            "dangling schema reference: {reference}"
                        );
                    }
                    for value in object.values() {
                        references(value, root);
                    }
                }
                Value::Array(values) => {
                    for value in values {
                        references(value, root);
                    }
                }
                _ => (),
            }
        }
        references(&document, &document);
        let schemas = document["components"]["schemas"].as_object().unwrap();
        assert!(schemas.contains_key("MessageDetail"));
        for platform in [
            "Generic",
            "Github",
            "Gitea",
            "AppStoreConnect",
            "Gitlab",
            "Stripe",
            "Slack",
            "Feishu",
            "Telegram",
            "Shopify",
        ] {
            let name = format!("{platform}Webhook");
            let wrapper = &schemas[&name];
            assert_eq!(wrapper["required"], json!(["header", "payload"]));
            assert_eq!(wrapper["properties"].as_object().unwrap().len(), 2);
            assert_eq!(
                wrapper["properties"]["header"]["$ref"],
                format!("#/components/schemas/{platform}WebhookHeader")
            );
            assert_eq!(
                wrapper["properties"]["payload"]["$ref"],
                format!("#/components/schemas/{platform}WebhookPayload")
            );
        }
        for name in [
            "TelegramWebhookPayload",
            "TelegramMessage",
            "TelegramUser",
            "TelegramChat",
            "TelegramMaybeInaccessibleMessage",
            "TelegramInaccessibleMessage",
            "TelegramCallbackQuery",
            "TelegramMessageEntity",
            "TelegramPhotoSize",
            "TelegramPoll",
            "TelegramChatMember",
            "TelegramBusinessConnection",
            "TelegramSuccessfulPayment",
            "TelegramRichText",
            "TelegramRichBlock",
        ] {
            assert!(
                schemas.contains_key(name),
                "missing Telegram schema: {name}"
            );
        }
        for field in [
            "update_id",
            "message",
            "edited_message",
            "channel_post",
            "edited_channel_post",
            "business_connection",
            "business_message",
            "edited_business_message",
            "deleted_business_messages",
            "guest_message",
            "message_reaction",
            "message_reaction_count",
            "inline_query",
            "chosen_inline_result",
            "callback_query",
            "widget_query",
            "shared_widget_query",
            "shipping_query",
            "pre_checkout_query",
            "purchased_paid_media",
            "poll",
            "poll_answer",
            "my_chat_member",
            "chat_member",
            "chat_join_request",
            "chat_boost",
            "removed_chat_boost",
            "managed_bot",
            "subscription",
            "stopped_message_generation",
        ] {
            assert!(
                schemas["TelegramWebhookPayload"]["properties"]
                    .get(field)
                    .is_some(),
                "missing Telegram Update field: {field}"
            );
        }
        assert_eq!(
            schemas["TelegramChat"]["properties"]["id"]["format"],
            "int64"
        );
        assert_eq!(
            schemas["TelegramUser"]["properties"]["id"]["format"],
            "int64"
        );
        assert_eq!(
            schemas["TelegramCallbackQuery"]["properties"]["chat_instance"]["type"],
            json!(["string", "null"])
        );
        assert_eq!(
            schemas["TelegramMessage"]["properties"]["photo"]["items"]["$ref"],
            "#/components/schemas/TelegramPhotoSize"
        );
        assert_eq!(
            schemas["TelegramWebhookPayload"]["required"],
            json!(["update_id"])
        );
        for field in ["chat_id", "message_id", "callback_query_id", "update"] {
            assert!(
                schemas["TelegramWebhookPayload"]["properties"]
                    .get(field)
                    .is_none(),
                "unexpected synthetic Telegram field: {field}"
            );
        }
        assert!(!schemas.contains_key("TelegramUpdate"));
        for (name, field) in [
            ("GithubWebhookPayload", "commits"),
            ("GiteaChanges", "removed_labels"),
            ("AppStoreConnectAttributes", "territories"),
            ("TelegramMessage", "entities"),
        ] {
            let property = &schemas[name]["properties"][field];
            assert_eq!(property["type"], json!(["array", "null"]));
            assert!(
                property.get("default").is_none(),
                "missing {name}.{field} must not acquire a synthetic default"
            );
            assert!(
                !schemas[name]["required"]
                    .as_array()
                    .is_some_and(|required| required.contains(&json!(field)))
            );
        }
        assert_eq!(
            schemas["AppStoreConnectWebhookPayload"]["required"],
            json!(["data"])
        );
        assert_eq!(
            schemas["AppStoreConnectNotification"]["required"],
            json!(["type"])
        );
        for name in [
            "AppStoreConnectWebhookPayload",
            "AppStoreConnectNotification",
            "AppStoreConnectAttributes",
            "AppStoreConnectRelationship",
            "AppStoreConnectCredentials",
        ] {
            assert!(
                schemas.contains_key(name),
                "missing App Store Connect schema: {name}"
            );
        }
        let attributes = &schemas["AppStoreConnectAttributes"]["properties"];
        for field in [
            "newValue",
            "oldValue",
            "newExternalBuildState",
            "oldExternalBuildState",
            "newState",
            "oldState",
            "timestamp",
            "available",
            "territories",
            "appId",
        ] {
            assert!(
                attributes.get(field).is_some(),
                "missing App Store Connect attribute: {field}"
            );
        }
        assert!(
            schemas["AppStoreConnectRelationship"]["properties"]
                .get("data")
                .is_some()
        );
        assert!(
            schemas["AppStoreConnectRelationship"]["properties"]
                .get("id")
                .is_some()
        );
        for name in [
            "GiteaWebhookPayload",
            "GiteaCredentials",
            "GiteaReview",
            "GiteaPackage",
            "GiteaWorkflowRun",
        ] {
            assert!(schemas.contains_key(name), "missing Gitea schema: {name}");
        }
        assert_eq!(
            schemas["GiteaWorkflow"]["properties"]["id"]["type"],
            json!(["string", "null"])
        );
        assert_eq!(schemas["GiteaUser"]["properties"]["id"]["format"], "int64");
        for field in [
            "ref",
            "commits",
            "state",
            "page",
            "forkee",
            "package",
            "organization",
            "requested_reviewer",
            "is_pull",
        ] {
            assert!(
                schemas["GiteaWebhookPayload"]["properties"]
                    .get(field)
                    .is_some(),
                "missing Gitea field: {field}"
            );
        }
        for field in [
            "review",
            "thread",
            "workflow_job",
            "check_run",
            "check_suite",
            "release",
            "discussion",
            "deployment",
            "deployment_status",
            "changes",
            "ref",
            "commits",
            "state",
        ] {
            assert!(
                schemas["GithubWebhookPayload"]["properties"]
                    .get(field)
                    .is_some(),
                "missing GitHub event field: {field}"
            );
        }
        for platform in ["Github", "Gitea"] {
            let name = format!("{platform}WebhookPayload");
            for field in [
                "push",
                "git_ref",
                "commit_status",
                "wiki",
                "event_type",
                "installation_target_type",
            ] {
                assert!(
                    schemas[&name]["properties"].get(field).is_none(),
                    "synthetic payload field {name}.{field}"
                );
            }
        }
        for name in [
            "GithubWebhookHeader",
            "GiteaWebhookHeader",
            "GitlabWebhookHeader",
            "SlackWebhookHeader",
            "ShopifyWebhookHeader",
            "TelegramWebhookHeader",
        ] {
            for field in ["signature", "token", "secret_token", "authorization"] {
                assert!(schemas[name]["properties"].get(field).is_none());
            }
        }
        assert!(
            schemas["GithubComment"]["properties"]
                .get("in_reply_to_id")
                .is_some()
        );
        assert!(
            schemas["GithubIssue"]["properties"]
                .get("pull_request")
                .is_some()
        );
        assert!(
            schemas["GithubWorkflowJob"]["properties"]
                .get("steps")
                .is_some()
        );
        assert!(schemas["GithubTimestamp"]["anyOf"].is_array());
        assert!(
            schemas
                .keys()
                .filter(|name| name.starts_with("Page_"))
                .count()
                >= 2
        );
        assert_eq!(
            schemas["MessageListItem"]["properties"]["source"]["type"],
            "string"
        );
    }
}
