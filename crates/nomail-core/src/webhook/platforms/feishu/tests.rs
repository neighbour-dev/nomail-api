use crate::webhook::{test_support::*, *};
use aes::cipher::{BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn encrypt_feishu(payload: &[u8]) -> Vec<u8> {
    let key = Sha256::digest(b"encrypt-key");
    let iv = [7; 16];
    let ciphertext = cbc::Encryptor::<aes::Aes256>::new_from_slices(&key, &iv)
        .unwrap()
        .encrypt_padded_vec_mut::<Pkcs7>(payload);
    let mut buffer = iv.to_vec();
    buffer.extend(ciphertext);
    serde_json::to_vec(&json!({"encrypt":STANDARD.encode(buffer)})).unwrap()
}

fn feishu_headers(body: &[u8]) -> Vec<(String, String)> {
    let mut hash = Sha256::new();
    hash.update(format!("{NOW}nonceencrypt-key"));
    hash.update(body);
    headers(&[
        ("X-Lark-Request-Timestamp", &NOW.to_string()),
        ("X-Lark-Request-Nonce", "nonce"),
        ("X-Lark-Signature", &hex::encode(hash.finalize())),
    ])
}

fn feishu_auth() -> Authentication<'static> {
    Authentication::Feishu {
        verification_token: Some("verification-token"),
        encrypt_key: Some("encrypt-key"),
    }
}

#[test]
fn encrypted_v2_and_nested_text() {
    let plain = br#"{"schema":"2.0","header":{"event_id":"event-1","event_type":"im.message.receive_v1"},"event":{"message":{"message_type":"text","content":"{\"text\":\"hello\"}"}}}"#;
    let body = encrypt_feishu(plain);
    let h = feishu_headers(&body);
    let parsed = event(receive_with(Provider::Feishu, feishu_auth(), &h, &body).unwrap());
    assert!(matches!(
        parsed.data,
        PlatformData::Feishu(FeishuEvent::V2(_))
    ));
    assert_eq!(
        parsed.raw.as_json(),
        Some(&serde_json::from_slice::<Value>(plain).unwrap())
    );
    assert!(parsed.raw.as_json().unwrap().get("encrypt").is_none());
    assert!(
        parsed
            .raw
            .to_readable_string()
            .contains("im.message.receive_v1")
    );
    assert_eq!(parsed.to_message_content().content, "hello");
    assert_eq!(
        parsed.metadata().deduplication_key.as_ref().unwrap().value,
        "event-1"
    );
    assert_eq!(
        receive_with(
            Provider::Feishu,
            feishu_auth(),
            &feishu_headers(plain),
            &body
        )
        .unwrap_err(),
        WebhookError::Unauthorized
    );
    assert_eq!(
        receive_with(Provider::Feishu, feishu_auth(), &Vec::new(), plain).unwrap_err(),
        WebhookError::Unauthorized
    );
    let configured = PlatformConfig::Feishu(FeishuCredentials {
        verification_token: "verification-token".into(),
        encrypt_key: Some("encrypt-key".into()),
    });
    let configured = configured.receiving_config(NOW).unwrap();
    assert!(
        receive(
            &configured,
            &WebhookRequest {
                headers: &h,
                body: &body
            }
        )
        .is_ok()
    );
    // Even a correctly signed plaintext event cannot disable configured encryption.
    assert!(matches!(
        receive(
            &configured,
            &WebhookRequest {
                headers: &feishu_headers(plain),
                body: plain
            }
        ),
        Err(WebhookError::Unauthorized)
    ));
}

#[test]
fn encrypted_and_plain_challenges_use_separate_token() {
    let plain = br#"{"type":"url_verification","token":"verification-token","challenge":"challenge-value"}"#;
    for body in [plain.to_vec(), encrypt_feishu(plain)] {
        let reply =
            response(receive_with(Provider::Feishu, feishu_auth(), &Vec::new(), &body).unwrap());
        assert_eq!(
            serde_json::from_slice::<Value>(&reply.body).unwrap(),
            json!({"challenge":"challenge-value"})
        );
    }
    let wrong =
        encrypt_feishu(br#"{"type":"url_verification","token":"wrong","challenge":"value"}"#);
    assert_eq!(
        receive_with(Provider::Feishu, feishu_auth(), &Vec::new(), &wrong).unwrap_err(),
        WebhookError::Unauthorized
    );
    assert!(
        receive_with(
            Provider::Feishu,
            Authentication::Feishu {
                verification_token: None,
                encrypt_key: Some("encrypt-key")
            },
            &Vec::new(),
            &encrypt_feishu(plain)
        )
        .is_err()
    );
}

#[test]
fn plaintext_token_auth_and_legacy_events() {
    let body = br#"{"uuid":"event-1","type":"legacy_event","token":"verification-token","event":{"content":"hello"}}"#;
    let parsed = event(
        receive_with(
            Provider::Feishu,
            Authentication::Feishu {
                verification_token: Some("verification-token"),
                encrypt_key: None,
            },
            &Vec::new(),
            body,
        )
        .unwrap(),
    );
    assert!(matches!(
        parsed.data,
        PlatformData::Feishu(FeishuEvent::Legacy(_))
    ));
    assert_eq!(parsed.to_message_content().content, "hello");
    assert_eq!(
        receive_with(
            Provider::Feishu,
            Authentication::Feishu {
                verification_token: Some("wrong"),
                encrypt_key: None
            },
            &Vec::new(),
            body
        )
        .unwrap_err(),
        WebhookError::Unauthorized
    );
}

#[test]
fn malformed_encryption_and_unknown_version_fail() {
    for body in [
        br#"{"encrypt":"bad-base64"}"#.as_slice(),
        br#"{"encrypt":"YQ=="}"#,
    ] {
        assert!(receive_with(Provider::Feishu, feishu_auth(), &Vec::new(), body).is_err());
    }
    let body = br#"{"schema":"3.0","header":{"event_type":"future"}}"#;
    assert!(matches!(
        receive_with(Provider::Feishu, Authentication::None, &Vec::new(), body),
        Err(WebhookError::Unsupported(_))
    ));
}

#[test]
fn timestamp_is_enforced() {
    let body = encrypt_feishu(br#"{"schema":"2.0","header":{"event_type":"message"},"event":{}}"#);
    let mut h = feishu_headers(&body);
    h.iter_mut()
        .find(|(name, _)| name == "X-Lark-Request-Timestamp")
        .unwrap()
        .1 = (NOW + 301).to_string();
    assert_eq!(
        receive_with(Provider::Feishu, feishu_auth(), &h, &body).unwrap_err(),
        WebhookError::Unauthorized
    );
}

#[test]
fn unsigned_encrypted_failures_do_not_expose_padding_or_json_errors() {
    for body in [
        br#"{"encrypt":"invalid"}"#.to_vec(),
        encrypt_feishu(b"not json"),
        encrypt_feishu(br#"{"type":"event","event":{}}"#),
        encrypt_feishu(br#"{"type":"url_verification","token":"wrong","challenge":"value"}"#),
    ] {
        assert_eq!(
            receive_with(Provider::Feishu, feishu_auth(), &Vec::new(), &body).unwrap_err(),
            WebhookError::Unauthorized
        );
    }
}
