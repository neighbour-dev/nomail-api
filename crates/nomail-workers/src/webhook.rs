pub mod configs;
use crate::{
    config::Config,
    db::Store,
    error::{Result, success},
    time,
};
use chrono::{DateTime, Utc};
use nomail_app::{
    http::{endpoints as e, webhook::WebhookReply},
    ingestion::{IngestionError, IngestionResult, IngestionRuntime, IngestionService},
    webhook::WebhookRequest,
};
use worker::{Env, Request, Response};
struct Runtime<'a>(&'a [u8; 32]);
impl IngestionRuntime for Runtime<'_> {
    fn now(&self) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(time::now_ms()).expect("runtime timestamp in range")
    }
    fn message_id(&self, now: DateTime<Utc>) -> IngestionResult<uuid::Uuid> {
        crate::crypto::message_id(now.timestamp_millis() as u64)
            .map_err(|_| IngestionError::Internal("message identity generation failed".into()))
    }
    fn encryption_key(&self) -> &[u8; 32] {
        self.0
    }
}
pub async fn receive(
    req: &Request,
    bytes: &[u8],
    key: &str,
    db: &Store,
    config: &Config,
    env: &Env,
) -> Result<Response> {
    let headers: Vec<(String, String)> = req.headers().entries().collect();
    let outcome = IngestionService {
        messages: crate::message::service(db),
        runtime: Runtime(&config.encryption_key),
    }
    .receive(
        key,
        &WebhookRequest {
            headers: &headers,
            body: bytes,
        },
    )
    .await?;
    match WebhookReply::from(outcome) {
        WebhookReply::Control(response) => {
            let mut result = Response::from_bytes(response.body)?.with_status(response.status);
            result
                .headers_mut()
                .set("Content-Type", &response.content_type)?;
            Ok(result)
        }
        WebhookReply::Acknowledgement { response, created } => {
            if let Some((user_id, event)) = created
                && let Err(_error) = crate::realtime::publish(env, user_id, event).await
            {
                worker::console_error!("realtime publish failed: internal operation failed");
            }
            success::<e::ReceiveWebhook>(response)
        }
    }
}
