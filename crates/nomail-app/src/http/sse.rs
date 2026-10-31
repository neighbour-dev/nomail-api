//! Public online event protocol. Connections/fanout/timers remain adapters.
use crate::{message::MessageAccepted, webhook::MessageLevel};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const HEARTBEAT_SECONDS: u64 = 20;
pub const HEARTBEAT: &[u8] = b": heartbeat\n\n";

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
    pub title: String,
    pub source: String,
    pub level: MessageLevel,
    pub status: String,
    pub pinned: bool,
    pub webhook_config_id: i64,
    pub created_at: DateTime<Utc>,
    pub expire_at: Option<DateTime<Utc>>,
}
impl From<MessageAccepted> for MessageCreated {
    fn from(message: MessageAccepted) -> Self {
        Self {
            id: message.id,
            title: message.title,
            source: message.source,
            level: message.level,
            status: message.status,
            pinned: message.pinned,
            webhook_config_id: message.webhook_config_id,
            created_at: message.received_at,
            expire_at: message.expires_at,
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
            fn data_json(&self)->String {match self {$(Self::$variant(data)=>serde_json::to_string(data).expect("SSE event data serializes")),*}}
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
pub fn encode(event: &SseEvent) -> String {
    let id = match event {
        SseEvent::MessageCreated(data) => format!("id: {}\n", data.id),
        _ => String::new(),
    };
    format!(
        "{id}event: {}\ndata: {}\n\n",
        event.event_name(),
        event.data_json()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
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
            title: "line\nnot-an-event".into(),
            source: String::new(),
            level: MessageLevel::High,
            status: "read".into(),
            pinned: true,
            webhook_config_id: 1,
            created_at: now,
            expire_at: None,
        });
        let text = encode(&event);
        assert!(text.starts_with(&format!("id: {id}\nevent: message.created\ndata: ")));
        assert_eq!(text.lines().count(), 4);
        let decoded: SseEvent =
            serde_json::from_value(serde_json::to_value(event).unwrap()).unwrap();
        assert_eq!(decoded.event_name(), "message.created");
    }
}
