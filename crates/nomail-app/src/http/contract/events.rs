//! Public online event protocol. Connections/fanout/timers remain adapters.
use crate::message::MessageAccepted;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ReadyEvent {
    pub connected_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct EmptyEvent {}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct MessageCreated {
    pub id: Uuid,
    /// Server receipt time of the message, not the SSE delivery time (UTC).
    pub received_at: DateTime<Utc>,
}
impl From<MessageAccepted> for MessageCreated {
    fn from(message: MessageAccepted) -> Self {
        Self {
            id: message.id,
            received_at: message.received_at,
        }
    }
}
macro_rules! events {
    ($($variant:ident($data:ty)=$name:literal),* $(,)?)=>{
        #[derive(Clone,Debug,Serialize,Deserialize)]
        #[cfg_attr(feature="openapi",derive(schemars::JsonSchema))]
        #[serde(tag="event",content="data")]
        pub enum SseEvent {$ (#[serde(rename=$name)] $variant($data)),*}
        impl SseEvent {
            pub fn event_name(&self)->&'static str {match self {$(Self::$variant(_)=>$name),*}}
            pub(crate) fn data_json(&self)->String {match self {$(Self::$variant(data)=>serde_json::to_string(data).expect("SSE event data serializes")),*}}
        }
    }
}
events! {Ready(ReadyEvent)="ready",MessageCreated(MessageCreated)="message.created",AuthExpired(EmptyEvent)="auth.expired"}
impl SseEvent {
    pub fn ready(now: DateTime<Utc>) -> Self {
        Self::Ready(ReadyEvent { connected_at: now })
    }
    pub fn auth_expired() -> Self {
        Self::AuthExpired(EmptyEvent {})
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::sse::encode;
    #[test]
    fn event_names_payloads_and_frames_are_one_contract() {
        let now = DateTime::from_timestamp(1000, 0).unwrap();
        assert!(encode(&SseEvent::ready(now)).starts_with("event: ready\ndata: {\"connectedAt\":"));
        assert_eq!(
            encode(&SseEvent::auth_expired()),
            "event: auth.expired\ndata: {}\n\n"
        );
        let id = Uuid::from_u128(1);
        let event = SseEvent::MessageCreated(MessageCreated {
            id,
            received_at: now,
        });
        let text = encode(&event);
        assert_eq!(
            text,
            format!(
                "id: {id}\nevent: message.created\ndata: {{\"id\":\"{id}\",\"receivedAt\":\"1970-01-01T00:16:40Z\"}}\n\n"
            )
        );
        let decoded: SseEvent =
            serde_json::from_value(serde_json::to_value(event).unwrap()).unwrap();
        assert_eq!(decoded.event_name(), "message.created");
    }
}
