//! Unified discovery and handling contract, not a catch-all domain error.
//! Definitions remain owned by their respective business modules.
mod policy;
pub use crate::{
    auth::{AuthError, AuthResult},
    message::{MessageError, MessageResult},
    webhook::{
        WebhookError,
        config::{ConfigError, ConfigResult},
    },
};
pub use policy::*;

#[cfg(test)]
mod tests;
