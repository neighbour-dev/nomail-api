//! The complete public HTTP contract: routes, requests, responses and SSE events.

mod auth;
mod common;
mod device;
mod health;
mod message;
mod routes;
mod webhook;

pub use auth::{
    get_account::AccountResponse,
    login::{LoginRequest, TokenResponse},
    refresh::RefreshRequest,
    setup::SetupRequest,
    setup_status::SetupStatusResponse,
    update_account::AccountUpdateRequest,
};
pub use common::{ApiResponse, Page};
pub use device::{
    get_push_preferences::PushPreferences,
    list_devices::DeviceResponse,
    register_push::{PushChannel, RegisterPushRequest},
};
pub mod events {
    pub use super::message::stream_messages::{EmptyEvent, MessageCreated, ReadyEvent, SseEvent};
}
pub use health::{HealthResponse, HealthStatus};
pub use message::{
    batch_messages::{AffectedResponse, BatchMessagesRequest},
    cursor_messages::{CursorMessagesQuery, MessageCursorResponse},
    get_message::{MessageConfigSummary, MessageDetail, MessageMetadata},
    list_messages::{ListMessagesQuery, MessageListItem},
    message_filter::MessageFilterQuery,
    raw_message::MessageRawResponse,
    unread_count::CountResponse,
    update_message::UpdateMessageRequest,
};
pub use routes::*;
pub use webhook::{
    create_config::CreateConfigRequest, enable_config::EnabledRequest, get_config::ConfigResponse,
    list_configs::PaginationQuery, receive_webhook::ReceiveResponse,
    update_config::UpdateConfigRequest,
};

pub type EndpointResponse<E> = ApiResponse<<E as Endpoint>::Response>;
