//! Shared community authentication. HTTP, SQL and execution runtimes are adapters.
mod claims;
mod error;
mod input;
mod model;
pub mod password;
mod repository;
mod runtime;
mod service;
pub mod token;
mod view;

pub use claims::Claims;
pub use error::{AuthError, AuthResult};
pub use input::{InitializeAccount, Login, UpdateAccount, check_length};
pub use model::*;
pub use repository::*;
pub use runtime::AuthRuntime;
pub use service::AuthService;
pub use view::{AccountInfo, DeviceInfo, SessionTokens};

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
}
