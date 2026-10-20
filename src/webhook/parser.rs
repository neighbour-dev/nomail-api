use crate::error::ApiError;
use actix_web::http::header::HeaderMap;
use hmac::{Hmac, Mac};
use serde_json::Value;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

#[derive(Default, Debug)]
pub struct ParsedMessage {
    pub external_id: String,
    pub title: String,
    pub content: String,
    pub source: String,
    pub level: String,
    pub url: String,
    pub challenge: String,
}
pub fn provider(value: &str) -> Result<&'static str, ApiError> {
    match value.trim().to_lowercase().as_str() {
        "generic" => Ok("generic"),
        "github" => Ok("github"),
        "feishu" => Ok("feishu"),
        _ => Err(ApiError::bad("unsupported provider")),
    }
}
fn header<'a>(headers: &'a HeaderMap, key: &str) -> &'a str {
    headers.get(key).and_then(|v| v.to_str().ok()).unwrap_or("")
}
fn nested(value: &Value, path: &[&str]) -> String {
    let mut value = value;
    for key in path {
        value = &value[*key];
    }
    match value {
        Value::String(v) => v.trim().into(),
        Value::Number(v) => v.to_string(),
        Value::Bool(v) => v.to_string(),
        _ => String::new(),
    }
}
fn first(values: &[String]) -> String {
    values
        .iter()
        .find(|v| !v.trim().is_empty())
        .map(|v| v.trim().into())
        .unwrap_or_default()
}
fn first_str<'a>(values: &[&'a str]) -> &'a str {
    values
        .iter()
        .copied()
        .find(|v| !v.trim().is_empty())
        .unwrap_or("")
        .trim()
}

pub fn verify(
    provider: &str,
    headers: &HeaderMap,
    body: &[u8],
    secret: &str,
) -> Result<(), ApiError> {
    if secret.is_empty() {
        return Ok(());
    }
    let invalid = || ApiError::unauthorized();
    if provider == "feishu" {
        let signature = first_str(&[
            header(headers, "X-Lark-Signature"),
            header(headers, "X-Feishu-Signature"),
        ]);
        if !signature.is_empty() {
            let timestamp = first_str(&[
                header(headers, "X-Lark-Request-Timestamp"),
                header(headers, "X-Feishu-Request-Timestamp"),
            ]);
            let nonce = first_str(&[
                header(headers, "X-Lark-Request-Nonce"),
                header(headers, "X-Feishu-Request-Nonce"),
            ]);
            let mut digest = Sha256::new();
            digest.update(timestamp);
            digest.update(nonce);
            digest.update(secret);
            digest.update(body);
            let expected = hex::encode(digest.finalize());
            if !bool::from(expected.as_bytes().ct_eq(signature.as_bytes())) {
                return Err(invalid());
            }
        } else {
            let payload: Value = serde_json::from_slice(body).map_err(|_| invalid())?;
            let token = first(&[
                nested(&payload, &["token"]),
                nested(&payload, &["header", "token"]),
            ]);
            if !bool::from(token.as_bytes().ct_eq(secret.as_bytes())) {
                return Err(invalid());
            }
        }
        return Ok(());
    }
    let key = if provider == "github" {
        "X-Hub-Signature-256"
    } else {
        "X-NoMail-Signature"
    };
    let signature = header(headers, key)
        .strip_prefix("sha256=")
        .ok_or_else(invalid)?;
    let signature = hex::decode(signature).map_err(|_| invalid())?;
    let mut mac =
        <Hmac<Sha256> as Mac>::new_from_slice(secret.as_bytes()).map_err(ApiError::internal)?;
    mac.update(body);
    mac.verify_slice(&signature).map_err(|_| invalid())
}

pub fn parse(provider: &str, headers: &HeaderMap, body: &[u8]) -> Result<ParsedMessage, ApiError> {
    let payload = serde_json::from_slice::<Value>(body)
        .ok()
        .filter(|v| v.is_object() || v.is_null());
    if provider == "generic" && payload.is_none() {
        let content = String::from_utf8_lossy(body).trim().to_owned();
        if content.is_empty() {
            return Err(ApiError::bad("empty payload"));
        }
        return Ok(ParsedMessage {
            external_id: header(headers, "X-NoMail-Idempotency-Key").into(),
            title: first_str(&[header(headers, "X-NoMail-Title"), "Webhook notification"]).into(),
            content,
            source: header(headers, "X-NoMail-Source").into(),
            level: header(headers, "X-NoMail-Level").into(),
            url: header(headers, "X-NoMail-URL").into(),
            ..Default::default()
        });
    }
    let p = payload.ok_or_else(|| ApiError::bad("invalid webhook payload"))?;
    match provider {
        "generic" => {
            let event_type = first(&[nested(&p, &["eventType"]), nested(&p, &["type"])]);
            Ok(ParsedMessage {
                external_id: first(&[
                    header(headers, "X-NoMail-Idempotency-Key").into(),
                    nested(&p, &["id"]),
                    nested(&p, &["eventId"]),
                ]),
                title: first(&[
                    nested(&p, &["title"]),
                    event_type,
                    "Webhook notification".into(),
                ]),
                content: first(&[
                    nested(&p, &["content"]),
                    nested(&p, &["message"]),
                    nested(&p, &["body"]),
                    p.to_string(),
                ]),
                source: nested(&p, &["source"]),
                level: nested(&p, &["level"]),
                url: nested(&p, &["url"]),
                ..Default::default()
            })
        }
        "github" => {
            let event = first_str(&[header(headers, "X-GitHub-Event"), "event"]);
            let mut title = format!("GitHub {event}");
            for value in [
                nested(&p, &["action"]),
                nested(&p, &["repository", "full_name"]),
            ] {
                if !value.is_empty() {
                    title.push_str(&format!(": {value}"));
                }
            }
            let conclusion = nested(&p, &["workflow_run", "conclusion"]);
            let level = match conclusion.as_str() {
                "failure" | "cancelled" | "timed_out" | "startup_failure" => "critical",
                "success" => "success",
                _ => "normal",
            };
            Ok(ParsedMessage {
                external_id: first(&[
                    header(headers, "X-GitHub-Delivery").into(),
                    nested(&p, &["id"]),
                ]),
                title,
                content: first(&[
                    nested(&p, &["head_commit", "message"]),
                    nested(&p, &["workflow_run", "name"]),
                    nested(&p, &["pull_request", "title"]),
                    nested(&p, &["issue", "title"]),
                    nested(&p, &["comment", "body"]),
                    p.to_string(),
                ]),
                source: first(&[
                    nested(&p, &["repository", "full_name"]),
                    nested(&p, &["sender", "login"]),
                    "github".into(),
                ]),
                level: level.into(),
                url: first(&[
                    nested(&p, &["workflow_run", "html_url"]),
                    nested(&p, &["pull_request", "html_url"]),
                    nested(&p, &["issue", "html_url"]),
                    nested(&p, &["comment", "html_url"]),
                    nested(&p, &["repository", "html_url"]),
                ]),
                ..Default::default()
            })
        }
        "feishu" => {
            let event = first(&[
                nested(&p, &["header", "event_type"]),
                nested(&p, &["type"]),
                "event".into(),
            ]);
            Ok(ParsedMessage {
                challenge: nested(&p, &["challenge"]),
                external_id: first(&[nested(&p, &["header", "event_id"]), nested(&p, &["uuid"])]),
                title: first(&[nested(&p, &["title"]), event]),
                content: first(&[
                    nested(&p, &["content"]),
                    nested(&p, &["event", "message", "content"]),
                    nested(&p, &["event", "content"]),
                    p["event"].to_string(),
                ]),
                source: first(&[
                    nested(&p, &["source"]),
                    nested(&p, &["header", "app_id"]),
                    "feishu".into(),
                ]),
                level: nested(&p, &["level"]),
                url: nested(&p, &["url"]),
            })
        }
        _ => Err(ApiError::bad("unsupported provider")),
    }
}

pub fn normalize_level(value: &str) -> &'static str {
    match value.trim().to_lowercase().as_str() {
        "critical" | "alert" | "error" | "failed" | "failure" => "critical",
        "warning" | "warn" => "warning",
        "success" | "pass" | "passed" | "ok" => "success",
        _ => "normal",
    }
}
pub fn retention_days(level: &str) -> i64 {
    match level {
        "critical" => 30,
        "warning" => 7,
        _ => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::http::header::{HeaderName, HeaderValue};
    fn headers(values: &[(&str, &str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in values {
            headers.insert(
                HeaderName::from_bytes(name.as_bytes()).unwrap(),
                HeaderValue::from_str(value).unwrap(),
            );
        }
        headers
    }
    #[test]
    fn generic_signed_json_and_plain_text() {
        let body =
            br#"{"id":"event-1","title":"Disk alert","content":"Usage is 92%","level":"critical"}"#;
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(b"secret").unwrap();
        mac.update(body);
        let signature = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));
        let headers = headers(&[("x-nomail-signature", &signature)]);
        verify("generic", &headers, body, "secret").unwrap();
        assert!(verify("generic", &headers, b"tampered", "secret").is_err());
        let parsed = parse("generic", &headers, body).unwrap();
        assert_eq!(parsed.external_id, "event-1");
        assert_eq!(parsed.level, "critical");
        assert_eq!(
            parse("generic", &headers, b" Backup done ")
                .unwrap()
                .content,
            "Backup done"
        );
        assert!(parse("generic", &headers, b" ").is_err());
    }
    #[test]
    fn github_fields_and_level() {
        let headers = headers(&[
            ("X-GitHub-Delivery", "delivery-1"),
            ("X-GitHub-Event", "workflow_run"),
        ]);
        let body = br#"{"repository":{"full_name":"owner/repo"},"workflow_run":{"name":"CI","conclusion":"failure","html_url":"https://example.com/run/1"}}"#;
        let parsed = parse("github", &headers, body).unwrap();
        assert_eq!(parsed.external_id, "delivery-1");
        assert_eq!(parsed.source, "owner/repo");
        assert_eq!(parsed.level, "critical");
        assert_eq!(parsed.title, "GitHub workflow_run: owner/repo");
    }
    #[test]
    fn feishu_token_challenge_and_signature() {
        let body = br#"{"challenge":"challenge-value","token":"verification-token"}"#;
        verify("feishu", &HeaderMap::new(), body, "verification-token").unwrap();
        assert_eq!(
            parse("feishu", &HeaderMap::new(), body).unwrap().challenge,
            "challenge-value"
        );
        let body = br#"{"header":{"event_id":"event-1","event_type":"im.message.receive_v1"},"event":{"message":{"content":"hello"}}}"#;
        let mut digest = Sha256::new();
        digest.update(b"1786968000nonce-valueencrypt-key");
        digest.update(body);
        let signature = hex::encode(digest.finalize());
        let headers = headers(&[
            ("X-Lark-Request-Timestamp", "1786968000"),
            ("X-Lark-Request-Nonce", "nonce-value"),
            ("X-Lark-Signature", &signature),
        ]);
        verify("feishu", &headers, body, "encrypt-key").unwrap();
        assert!(verify("feishu", &headers, body, "wrong-key").is_err());
        assert_eq!(
            parse("feishu", &headers, body).unwrap().external_id,
            "event-1"
        );
    }
    #[test]
    fn levels_and_providers() {
        assert_eq!(provider(" GitHub ").unwrap(), "github");
        assert!(provider("gitlab").is_err());
        assert_eq!(normalize_level(" ERROR "), "critical");
        assert_eq!(normalize_level("pass"), "success");
        assert_eq!(normalize_level("info"), "normal");
        assert_eq!(retention_days("critical"), 30);
    }
}
