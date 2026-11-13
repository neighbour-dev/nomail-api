use super::{StripeResource, StripeWebhookPayload, generated::RESOURCE_TAGS, support::StripeValue};
use crate::webhook::{test_support::*, *};
use serde_json::{Value, json};

fn stripe_headers(body: &[u8], time: i64) -> Vec<(String, String)> {
    let mut signed = format!("{time}.").into_bytes();
    signed.extend_from_slice(body);
    let signature = hex::encode(sign(b"whsec_literal-not-base64", &signed));
    headers(&[(
        "Stripe-Signature",
        &format!("t={time},v1=bad,v1={signature}"),
    )])
}

#[test]
fn snapshot_thin_and_rotating_signatures() {
    let snapshot = br#"{"id":"evt_1","object":"event","type":"payment_intent.succeeded","api_version":"2024-06-20","created":1710000000,"data":{"object":{"id":"pi_1"}}}"#;
    let thin = br#"{"id":"evt_2","object":"v2.core.event","type":"v2.core.account.updated","created":"2026-03-09T13:00:28Z","related_object":{"id":"acct_1","url":"/v2/accounts/acct_1"}}"#;
    for (body, is_thin) in [(snapshot.as_slice(), false), (thin.as_slice(), true)] {
        let h = stripe_headers(body, NOW);
        let parsed = event(
            receive_with(
                Provider::Stripe,
                Authentication::StripeSigningSecret("whsec_literal-not-base64"),
                &h,
                body,
            )
            .unwrap(),
        );
        assert_eq!(
            matches!(&parsed.data, Some(PlatformData::Stripe(webhook)) if webhook.payload.is_thin()),
            is_thin
        );
        if is_thin {
            assert!(
                parsed
                    .to_message_content()
                    .content
                    .contains("details not fetched")
            );
            assert!(parsed.to_message_content().url.is_none());
        } else {
            assert_eq!(parsed.to_message_content().level, MessageLevel::Normal);
        }
        assert!(
            receive_with(
                Provider::Stripe,
                Authentication::StripeSigningSecret("whsec_literal-not-base64"),
                &h,
                b"{}"
            )
            .is_err()
        );
    }
}

#[test]
fn payment_outcomes_do_not_assign_attention_priority() {
    for kind in ["payment_intent.failed", "payment_intent.succeeded"] {
        let body = serde_json::json!({"id":"evt_priority","object":"event","type":kind,"data":{"object":{"id":"pi_1"}}}).to_string();
        let parsed = event(
            receive_with(
                Provider::Stripe,
                Authentication::None,
                &Vec::new(),
                body.as_bytes(),
            )
            .unwrap(),
        );
        assert_eq!(parsed.to_message_content().level, MessageLevel::Normal);
        assert_eq!(parsed.metadata.event_type, kind);
    }
}

#[test]
fn timestamp_window_checks_past_and_future() {
    let body = br#"{"id":"evt_1","type":"test","object":"event"}"#;
    for time in [NOW - 301, NOW + 301] {
        assert_eq!(
            receive_with(
                Provider::Stripe,
                Authentication::StripeSigningSecret("whsec_literal-not-base64"),
                &stripe_headers(body, time),
                body
            )
            .unwrap_err(),
            WebhookError::Unauthorized
        );
    }
    assert!(
        receive_with(
            Provider::Stripe,
            Authentication::StripeSigningSecret("whsec_literal-not-base64"),
            &stripe_headers(body, NOW - 300),
            body
        )
        .is_ok()
    );
}

#[test]
fn typed_configuration_accepts_rotation_keys_but_never_unsigned_or_stale_deliveries() {
    let config = PlatformConfig::Stripe(StripeCredentials {
        signing_secrets: vec!["whsec_retired".into(), "whsec_literal-not-base64".into()],
    });
    let config = config.receiving_config(NOW).unwrap();
    let body = br#"{"id":"evt_rotation","type":"test","object":"event"}"#;
    assert!(
        receive(
            &config,
            &WebhookRequest {
                headers: &stripe_headers(body, NOW),
                body
            }
        )
        .is_ok()
    );
    for headers in [
        Vec::new(),
        stripe_headers(body, NOW - 301),
        stripe_headers(b"{}", NOW),
    ] {
        assert!(matches!(
            receive(
                &config,
                &WebhookRequest {
                    headers: &headers,
                    body
                }
            ),
            Err(WebhookError::Unauthorized)
        ));
    }
}

#[test]
fn unknown_envelopes_are_retained_without_guessing_a_new_event_type() {
    let parsed = event(
        receive_with(
            Provider::Stripe,
            Authentication::None,
            &Vec::new(),
            br#"{"id":"evt_1","type":"test","object":"unknown"}"#,
        )
        .unwrap(),
    );
    assert_eq!(parsed.raw.as_json().unwrap()["object"], "unknown");
    assert_eq!(parsed.metadata.event_type, "test");
    assert!(parsed.data.is_none());
}

fn signed_event(payload: &Value) -> PlatformEvent {
    let body = serde_json::to_vec(payload).unwrap();
    let parsed = event(
        receive_with(
            Provider::Stripe,
            Authentication::StripeSigningSecret("whsec_literal-not-base64"),
            &stripe_headers(&body, NOW),
            &body,
        )
        .unwrap(),
    );
    assert_eq!(parsed.raw.as_json(), Some(payload));
    assert!(
        parsed
            .raw
            .header
            .iter()
            .all(|h| !h.name.eq_ignore_ascii_case("stripe-signature"))
    );
    let restored = parse_raw(Provider::Stripe, &parsed.raw).ok();
    assert_eq!(restored, parsed.data);
    let encoded = serde_json::to_value(&parsed).unwrap();
    assert_eq!(
        serde_json::from_value::<PlatformEvent>(encoded).unwrap(),
        parsed
    );
    parsed
}

fn snapshot(resource: Value) -> PlatformEvent {
    signed_event(&json!({
        "id":"evt_example", "object":"event", "type":"example.updated",
        "api_version":super::STRIPE_MODEL_API_VERSION, "created":1790000000,
        "livemode":false, "pending_webhooks":0, "request":null,
        "data":{"object":resource}
    }))
}

fn projected(parsed: &PlatformEvent) -> Value {
    serde_json::to_value(&parsed.data).unwrap()["event"]["payload"].clone()
}

#[test]
fn every_generated_resource_dispatches_and_round_trips_without_synthetic_tags() {
    assert_eq!(RESOURCE_TAGS.len(), 200);
    for &(tag, deleted) in RESOURCE_TAGS {
        let mut object = json!({"object":tag});
        if deleted {
            object["deleted"] = json!(true);
        }
        let typed = StripeResource::read(&object).unwrap();
        assert!(
            !matches!(typed, StripeResource::Unknown(_)),
            "{tag}, deleted={deleted}"
        );
        assert_eq!(serde_json::to_value(&typed).unwrap(), object);
        assert_eq!(
            serde_json::from_value::<StripeResource>(object).unwrap(),
            typed
        );
    }
}

#[test]
fn payment_intent_models_money_expansions_errors_and_payment_method_details() {
    let resource = json!({
        "object":"payment_intent", "id":"pi_example", "amount":9007199254740993_i64,
        "amount_received":0, "currency":"usd", "status":"future_status",
        "customer":{"object":"customer", "id":"cus_example", "email":null,
            "metadata":{}, "invoice_settings":{"default_payment_method":"pm_example"}},
        "latest_charge":"ch_example", "payment_method":null,
        "last_payment_error":{"type":"card_error", "code":"card_declined",
            "decline_code":"insufficient_funds", "payment_method":{
                "object":"payment_method", "id":"pm_example", "type":"card",
                "card":{"brand":"visa", "last4":"4242", "exp_month":12, "exp_year":2030,
                    "checks":{"cvc_check":"pass"}}}},
        "metadata":{"empty":"", "verbatim":"  value  "},
        "payment_method_types":[], "description":"  not trimmed  "
    });
    let parsed = snapshot(resource.clone());
    assert_eq!(projected(&parsed)["data"]["object"], resource);
    let Some(PlatformData::Stripe(webhook)) = &parsed.data else {
        panic!("Stripe model missing")
    };
    let StripeWebhookPayload::Snapshot(event) = &webhook.payload else {
        panic!("wrong envelope")
    };
    let StripeResource::PaymentIntent(intent) = event.data.value().unwrap().object.value().unwrap()
    else {
        panic!("wrong resource")
    };
    assert_eq!(intent.amount.value(), Some(&9007199254740993));
    assert_eq!(intent.amount_received.value(), Some(&0));
    assert_eq!(intent.status.value_deref(), Some("future_status"));
}

#[test]
fn invoice_subscription_checkout_and_decimal_strings_keep_native_paths() {
    for resource in [
        json!({"object":"invoice", "id":"in_example", "amount_due":2000, "amount_paid":0,
            "parent":{"type":"subscription_details", "subscription_details":{"subscription":"sub_example", "metadata":{}}},
            "lines":{"object":"list", "data":[{"object":"line_item", "id":"il_example", "amount":2000,
                "quantity":2, "currency":"usd", "period":{"start":0,"end":1790000000}}], "has_more":false,"url":"/v1/invoices/in_example/lines"},
            "status_transitions":{"finalized_at":null,"paid_at":null}, "metadata":{}}),
        json!({"object":"subscription", "id":"sub_example", "status":"active", "customer":"cus_example",
            "items":{"object":"list", "data":[{"object":"subscription_item", "id":"si_example", "quantity":0,
                "current_period_start":1790000000, "current_period_end":1792678400,
                "price":{"object":"price", "id":"price_example", "unit_amount":null,
                    "unit_amount_decimal":"0.123456789012", "currency":"usd", "product":"prod_example",
                    "recurring":{"interval":"month","interval_count":1}}}],"has_more":false,"url":"/v1/subscription_items"}}),
        json!({"object":"checkout.session", "id":"cs_example", "mode":"payment", "payment_status":"paid",
            "amount_total":2000, "customer_details":{"email":"buyer@example.invalid", "tax_exempt":"none", "tax_ids":[]},
            "total_details":{"amount_discount":0,"amount_shipping":0,"amount_tax":0}, "payment_intent":"pi_example"}),
    ] {
        let parsed = snapshot(resource.clone());
        assert_eq!(projected(&parsed)["data"]["object"], resource);
    }
}

#[test]
fn expanded_deleted_objects_and_external_account_unions_are_not_first_match_structs() {
    for customer in [
        json!("cus_example"),
        json!({"object":"customer", "id":"cus_example", "name":"Example"}),
        json!({"object":"customer", "id":"cus_example", "deleted":true}),
        json!({"object":"future_customer", "id":"cus_example", "future":[null,false,0]}),
    ] {
        let resource = json!({"object":"payment_intent", "customer":customer});
        assert_eq!(
            projected(&snapshot(resource.clone()))["data"]["object"],
            resource
        );
    }
    for source in [
        json!({"object":"bank_account", "id":"ba_example", "account_holder_type":"company", "last4":"6789"}),
        json!({"object":"card", "id":"card_example", "brand":"Visa", "last4":"4242"}),
        json!({"object":"source", "id":"src_example", "type":"card", "amount":0}),
    ] {
        let resource = json!({"object":"customer", "default_source":source});
        assert_eq!(
            projected(&snapshot(resource.clone()))["data"]["object"],
            resource
        );
    }
}

#[test]
fn thin_notifications_keep_reason_related_object_and_snapshot_link_without_fetching() {
    let payload = json!({
        "id":"evt_thin", "object":"v2.core.event", "type":"v1.invoice.paid", "livemode":false,
        "created":"2026-11-13T01:02:03.456Z", "context":null, "snapshot_event":"evt_snapshot",
        "reason":{"type":"request", "request":{"id":"req_example", "idempotency_key":"  example  "}},
        "related_object":{"id":"in_example", "type":"invoice", "url":"/v1/invoices/in_example"}
    });
    let parsed = signed_event(&payload);
    assert_eq!(projected(&parsed), payload);
    assert!(
        parsed
            .to_message_content()
            .content
            .contains("details not fetched")
    );
    assert!(parsed.to_message_content().url.is_none());
    assert!(!projected(&parsed).as_object().unwrap().contains_key("data"));
    assert_eq!(parsed.metadata.version, None);
    // Thin event type prefixes do not determine the envelope. v1 resources can
    // produce thin events; object=v2.core.event is the format discriminator.
    assert_eq!(parsed.metadata.event_type, "v1.invoice.paid");
}

#[test]
fn malformed_fields_do_not_coerce_types_drop_list_positions_or_hide_valid_siblings() {
    let resource = json!({"object":"payment_intent", "id":"pi_example", "amount":"2000",
        "amount_received":false, "amount_capturable":0, "currency":" usd ",
        "description":false, "livemode":false, "payment_method_types":["card",null,"link"],
        "shipping":{"name":" Example ", "phone":123, "address":{"line1":"", "country":"US"}},
        "metadata":{"bad":1}, "future_field":{"keep":[null, false, 0]}});
    let parsed = snapshot(resource.clone());
    assert_eq!(
        projected(&parsed)["data"]["object"],
        json!({
            "object":"payment_intent", "id":"pi_example", "amount_capturable":0,
            "currency":" usd ", "livemode":false,
            "shipping":{"name":" Example ", "address":{"line1":"", "country":"US"}}
        })
    );
    assert_eq!(parsed.raw.as_json().unwrap()["data"]["object"], resource);
}

#[test]
fn presence_sparse_previous_values_and_unknown_objects_remain_lossless() {
    for resource in [
        json!({"object":"customer", "id":"cus_example", "email":null,"name":"", "balance":0,"delinquent":false,"metadata":{}}),
        json!({"id":"no_discriminator", "future":null}),
        json!({"object":"future_resource", "id":"future_example", "nested":[null,false,0,{},[]]}),
    ] {
        let payload = json!({"id":"evt_sparse", "object":"event", "type":"future_resource.updated", "request":{"id":null,"idempotency_key":null},
            "data":{"object":resource, "previous_attributes":{"email":null,"metadata":{"removed":null},"array":[null,{"old":false}]}}});
        assert_eq!(projected(&signed_event(&payload)), payload);
    }
}

#[test]
fn ambiguous_server_client_option_schemas_preserve_both_sets_of_fields() {
    let resource = json!({"object":"payment_intent", "payment_method_options":{"card":{
        "capture_method":"manual", "network":"visa", "request_three_d_secure":"automatic",
        "verification_method":"instant", "setup_future_usage":"off_session"
    }}});
    assert_eq!(
        projected(&snapshot(resource.clone()))["data"]["object"],
        resource
    );
}

#[test]
fn json_numbers_and_decimal_strings_are_distinct_wire_types() {
    let tax_rate = json!({"object":"tax_rate", "id":"txr_example", "percentage":12.345,
        "effective_percentage":0, "inclusive":false, "country":null});
    assert_eq!(
        projected(&snapshot(tax_rate.clone()))["data"]["object"],
        tax_rate
    );
    let price = json!({"object":"price", "id":"price_example", "unit_amount":0,
        "unit_amount_decimal":"0.000000000001", "active":false,
        "currency_options":{"usd":{"unit_amount":0,"unit_amount_decimal":"0.000000000001"}}});
    assert_eq!(projected(&snapshot(price.clone()))["data"]["object"], price);
    let malformed = json!({"object":"tax_rate", "percentage":"12.345", "inclusive":false});
    assert_eq!(
        projected(&snapshot(malformed))["data"]["object"],
        json!({"object":"tax_rate", "inclusive":false})
    );
}

#[test]
fn contradictory_nested_resource_discriminators_are_not_modeled_as_another_resource() {
    let resource = json!({"object":"subscription", "items":{"object":"list", "has_more":false,
        "data":[{"object":"price", "id":"price_not_a_subscription_item", "quantity":3}]}});
    let parsed = snapshot(resource.clone());
    assert_eq!(
        projected(&parsed)["data"]["object"],
        json!({"object":"subscription",
        "items":{"object":"list","has_more":false}})
    );
    assert_eq!(parsed.raw.as_json().unwrap()["data"]["object"], resource);
}

fn assert_projection_subset(projected: &Value, original: &Value) {
    match (projected, original) {
        (Value::Object(projected), Value::Object(original)) => {
            for (key, value) in projected {
                assert!(original.contains_key(key), "invented field: {key}");
                assert_projection_subset(value, &original[key]);
            }
        }
        (Value::Array(projected), Value::Array(original)) => {
            assert_eq!(projected.len(), original.len());
            for (a, b) in projected.iter().zip(original) {
                assert_projection_subset(a, b);
            }
        }
        _ => assert_eq!(projected, original),
    }
}

#[test]
fn official_resource_examples_preserve_modeled_values_and_all_raw_fields() {
    let fixtures: Value = serde_json::from_str(include_str!("fixtures/resources.json")).unwrap();
    let probes = [
        ("payment_intent", "amount"),
        ("charge", "payment_method_details"),
        ("checkout.session", "total_details"),
        ("customer", "invoice_settings"),
        ("subscription", "items"),
        ("invoice", "lines"),
        ("invoiceitem", "period"),
        ("refund", "amount"),
        ("dispute", "evidence_details"),
        ("payment_method", "card"),
        ("product", "marketing_features"),
        ("price", "unit_amount_decimal"),
        ("payout", "amount"),
        ("transfer", "reversals"),
        ("credit_note", "lines"),
        ("account", "capabilities"),
    ];
    for (kind, probe) in probes {
        let original = &fixtures[kind];
        let typed = StripeResource::read(original).unwrap();
        assert!(!matches!(typed, StripeResource::Unknown(_)), "{kind}");
        let result = projected(&snapshot(original.clone()))["data"]["object"].clone();
        assert_eq!(result["id"], original["id"], "{kind}");
        assert_eq!(result["object"], original["object"], "{kind}");
        assert!(
            result.as_object().unwrap().contains_key(probe),
            "missing {kind}.{probe}"
        );
        assert_projection_subset(&result, original);
    }
}

#[test]
fn shared_runtime_fixtures_have_the_same_signed_read_projection() {
    let fixtures: Value = serde_json::from_str(include_str!("fixtures/deliveries.json")).unwrap();
    for fixture in fixtures.as_array().unwrap() {
        let parsed = signed_event(&fixture["body"]);
        assert_eq!(
            projected(&parsed),
            fixture["projection"],
            "{}",
            fixture["name"]
        );
        assert_eq!(
            parsed.metadata.deduplication_key.as_ref().unwrap().scope,
            "stripe:event"
        );
    }
}
