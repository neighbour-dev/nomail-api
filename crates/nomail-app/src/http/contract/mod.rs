//! The complete public HTTP contract: routes, requests, responses and SSE events.
mod auth;
mod common;
mod config;
pub mod events;
mod message;
mod message_response;
mod routes;

pub use auth::*;
pub use common::*;
pub use config::*;
pub use message::*;
pub use message_response::*;
pub use routes::*;

pub type EndpointResponse<E> = ApiResponse<<E as Endpoint>::Response>;
