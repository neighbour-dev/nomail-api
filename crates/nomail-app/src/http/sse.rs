//! SSE framing; event wire types live with the HTTP contract.
pub use super::contract::events::*;

pub const HEARTBEAT_SECONDS: u64 = 20;
pub const HEARTBEAT: &[u8] = b": heartbeat\n\n";

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
