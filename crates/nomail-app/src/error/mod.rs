//! Unified discovery and handling contract, not a catch-all domain error.
//! Definitions remain owned by their respective business modules.
mod policy;
mod webhook;
pub use crate::ingestion::{IngestionError, IngestionResult};
pub use crate::{
    auth::{AuthError, AuthResult},
    device::{DeviceError, DeviceResult},
    message::{MessageError, MessageResult},
    webhook::{
        WebhookError,
        config::{ConfigError, ConfigResult},
    },
};
pub use policy::*;

pub(crate) fn validate_input(input: &impl garde::Validate<Context = ()>) -> Result<(), String> {
    input.validate().map_err(|report| report.to_string())
}

#[cfg(test)]
mod tests;
