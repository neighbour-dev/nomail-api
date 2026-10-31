//! Convert business outcomes into HTTP acknowledgements and optional SSE effects.
use super::{ReceiveResponse, sse::MessageCreated};
use crate::{ingestion::IngestionOutcome, webhook::ProtocolResponse};

pub enum WebhookReply {
    Control(ProtocolResponse),
    Acknowledgement {
        response: ReceiveResponse,
        created: Option<(i64, MessageCreated)>,
    },
}
impl From<IngestionOutcome> for WebhookReply {
    fn from(outcome: IngestionOutcome) -> Self {
        match outcome {
            IngestionOutcome::Control(response) => Self::Control(response),
            IngestionOutcome::Ignored => Self::Acknowledgement {
                response: ReceiveResponse::Ignored { ignored: true },
                created: None,
            },
            IngestionOutcome::Accepted {
                user_id,
                acceptance,
            } => Self::Acknowledgement {
                response: ReceiveResponse::Message {
                    message_id: acceptance.outcome.id(),
                    duplicate: acceptance.outcome.duplicate(),
                },
                created: acceptance
                    .created_event
                    .map(|event| (user_id, event.into())),
            },
        }
    }
}
