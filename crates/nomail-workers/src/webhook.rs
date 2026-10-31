pub mod configs;
use crate::{
    config::Config,
    crypto,
    db::Store,
    error::{Result, success},
    time,
};
use nomail_core::api::endpoints as e;
use nomail_core::webhook::{
    self, ReceiveOutcome, ReceiveResponse, WebhookConfig, WebhookRequest, service,
};
use serde_json::json;
use worker::{Env, Request, Response};

pub async fn receive(
    req: &Request,
    bytes: &[u8],
    key: &str,
    db: &Store,
    config: &Config,
    env: &Env,
) -> Result<Response> {
    let row = db
        .one(
            "SELECT * FROM webhook_configs WHERE key=? AND enabled=1 AND deleted_at IS NULL",
            vec![json!(key)],
        )
        .await?;
    let provider = service::configured_provider(row["provider"].as_str().unwrap())?;
    let encrypted = row["encrypted_secret"].as_str().unwrap_or("");
    let secret = if encrypted.is_empty() {
        String::new()
    } else {
        crypto::decrypt(encrypted, &config.encryption_key)?
    };
    let headers: Vec<(String, String)> = req.headers().entries().collect();
    let now_ms = time::now_ms();
    let outcome = webhook::receive(
        &WebhookConfig::new(
            provider,
            service::authentication(provider, &secret),
            now_ms / 1000,
        ),
        &WebhookRequest {
            headers: &headers,
            body: bytes,
        },
    )?;
    let event = match outcome {
        ReceiveOutcome::Event(event) => event,
        ReceiveOutcome::Respond(r) => {
            let mut response = Response::from_bytes(r.body)?.with_status(r.status);
            response
                .headers_mut()
                .set("Content-Type", &r.content_type)?;
            return Ok(response);
        }
        ReceiveOutcome::Ignored { .. } => {
            return success::<e::ReceiveWebhook>(ReceiveResponse::Ignored { ignored: true });
        }
    };
    let id = uuid::Uuid::parse_str(&crypto::message_id(now_ms as u64)?)
        .map_err(|e| worker::Error::RustError(e.to_string()))?;
    let now = chrono::DateTime::from_timestamp_millis(now_ms).expect("runtime timestamp in range");
    let accepted = crate::message::service(db)
        .accept(
            row["id"].as_i64().unwrap(),
            *event,
            row["source"].as_str(),
            id,
            now,
        )
        .await?;
    if let Some(created) = accepted.created_event
        && let Err(_error) =
            crate::realtime::publish(env, row["user_id"].as_i64().unwrap(), created).await
    {
        // Best-effort side effect has its own boundary; do not forward an
        // arbitrary runtime error (which may contain request data) to logs.
        worker::console_error!("realtime publish failed: internal operation failed");
    }
    success::<e::ReceiveWebhook>(ReceiveResponse::Message {
        message_id: accepted.outcome.id(),
        duplicate: accepted.outcome.duplicate(),
    })
}
