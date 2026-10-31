//! Shared community authentication. HTTP, SQL and execution runtimes are adapters.
mod dto;
mod error;
mod model;
pub mod password;
mod repository;
mod runtime;
mod service;
pub mod token;

pub use dto::*;
pub use error::{AuthError, AuthResult};
pub use model::*;
pub use repository::*;
pub use runtime::AuthRuntime;
pub use service::AuthService;

pub const ACCESS_TTL: i64 = 2 * 60 * 60;
pub const REFRESH_DAYS: i64 = 7;
pub const MAX_DEVICES: usize = 3;
pub const LAST_SEEN_SECONDS: i64 = 5 * 60;

#[cfg(test)]
mod tests;

/// Secret configuration is deliberately not Debug/Serialize.
pub struct AuthConfig {
    pub jwt_secret: String,
    pub jwt_issuer: String,
    pub initial_username: String,
    pub initial_password: String,
}
