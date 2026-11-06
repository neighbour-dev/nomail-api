//! Shared message use cases. Storage and HTTP are platform adapters.
mod command;
mod cursor;
mod error;
mod event;
mod model;
mod query;
mod repository;
mod service;
mod view;

pub use command::{BatchAction, BatchMessages, UpdateMessage};
pub use cursor::*;
pub use error::{MessageError, MessageResult};
pub use event::MessageAccepted;
pub use model::*;
pub use query::*;
pub use repository::*;
pub use service::MessageService;
pub use view::MessagePreview;

#[cfg(test)]
mod tests;

/// Temporary default until endpoint deletion policies exist. Priority does not
/// dictate retention, and no automatic expiry cleanup is implemented.
pub const DEFAULT_RETENTION_DAYS: i64 = 3;
