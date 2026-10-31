//! Shared message use cases. Storage and HTTP are platform adapters.
mod dto;
mod error;
mod model;
mod query;
mod repository;
mod service;

pub use dto::*;
pub use error::{MessageError, MessageResult};
pub use model::*;
pub use query::*;
pub use repository::*;
pub use service::MessageService;

#[cfg(test)]
mod tests;

/// Temporary default until endpoint deletion policies exist. Priority does not
/// dictate retention, and no automatic expiry cleanup is implemented.
pub const DEFAULT_RETENTION_DAYS: i64 = 3;
