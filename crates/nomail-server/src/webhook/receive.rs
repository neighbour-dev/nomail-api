use super::headers::Headers;
use crate::{
    AppState,
    error::{ApiError, ApiResult, success},
    message,
    realtime::Event,
};
use actix_web::{HttpRequest, HttpResponse, web};
use chrono::{DateTime, Utc};
use nomail_app::{
    http::{endpoints as e, webhook::WebhookReply},
    ingestion::{IngestionResult, IngestionRuntime, IngestionService},
    webhook::WebhookRequest,
};
struct Runtime<'a>(&'a [u8; 32]);
impl IngestionRuntime for Runtime<'_> {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
    fn message_id(&self, _now: DateTime<Utc>) -> IngestionResult<uuid::Uuid> {
        Ok(uuid::Uuid::now_v7())
    }
    fn encryption_key(&self) -> &[u8; 32] {
        self.0
    }
}
pub async fn handle(
    state: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
) -> ApiResult {
    let outcome = IngestionService {
        messages: message::service(&state.db),
        runtime: Runtime(&state.config.encryption_key),
    }
    .receive(
        &path,
        &WebhookRequest {
            headers: &Headers(req.headers()),
            body: &body,
        },
    )
    .await?;
    match WebhookReply::from(outcome) {
        WebhookReply::Control(response) => {
            let status = actix_web::http::StatusCode::from_u16(response.status)
                .map_err(ApiError::internal)?;
            Ok(HttpResponse::build(status)
                .content_type(response.content_type)
                .body(response.body))
        }
        WebhookReply::Acknowledgement { response, created } => {
            if let Some((user_id, event)) = created {
                state
                    .realtime
                    .publish(user_id, Event::MessageCreated(event));
            }
            Ok(success::<e::ReceiveWebhook>(response))
        }
    }
}
