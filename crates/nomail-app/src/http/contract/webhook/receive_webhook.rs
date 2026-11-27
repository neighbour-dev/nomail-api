use super::endpoint;
use crate::http::{Environment, HttpOutcome, HttpResult, dispatch::EndpointCall, transport::json};
use crate::http::{RealtimeEffect, webhook::WebhookReply};
use crate::webhook::WebhookRequest;
use serde::{Deserialize, Serialize};

endpoint! {
    ReceiveWebhook {
        method: Post,
        path: "/api/webhook/{key}",
        auth: Webhook,
        operation_id: "receiveWebhook",
        tag: "Webhook",
        summary: "接收平台消息或控制请求",
        request: Vec<u8>,
        query: (),
        response: ReceiveResponse,
        body_kind: Raw,
        response_kind: Webhook,
        has_query: false,
        path_kind: WebhookKey,
    }
}

impl ReceiveWebhook {
    pub(in crate::http) async fn handle(
        call: EndpointCall<'_, impl Environment>,
    ) -> HttpResult<HttpOutcome> {
        let outcome = call
            .services
            .ingestion
            .receive(
                call.matched.parameter()?,
                &WebhookRequest {
                    headers: call.head,
                    body: call.body,
                },
                call.environment.push_enabled(),
            )
            .await?;
        match WebhookReply::from(outcome) {
            WebhookReply::Control(response) => Ok(HttpOutcome::response(
                response.status,
                response.content_type,
                response.body,
            )),
            WebhookReply::Acknowledgement { response, created } => {
                let mut outcome = json::<Self>(response)?;
                if let Some((user_id, event)) = created {
                    outcome.events.push(RealtimeEffect { user_id, event });
                }
                Ok(outcome)
            }
        }
    }
}

/// NoMail HTTP acknowledgement. Platform controls keep their protocol response.
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum ReceiveResponse {
    Message {
        #[serde(rename = "messageId")]
        message_id: uuid::Uuid,
        duplicate: bool,
    },
    Ignored {
        ignored: bool,
    },
}
