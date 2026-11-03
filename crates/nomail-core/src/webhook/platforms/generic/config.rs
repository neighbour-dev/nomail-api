use crate::webhook::{Authentication, AuthenticationMode, WebhookError, config::secret};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub enum GenericCredentials {
    None {},
    HmacSha256 { secret: String },
}
impl GenericCredentials {
    pub fn validate(&self) -> Result<(), WebhookError> {
        match self {
            Self::None {} => Ok(()),
            Self::HmacSha256 { secret: value } => secret(value),
        }
    }
    pub fn mode(&self) -> AuthenticationMode {
        match self {
            Self::None {} => AuthenticationMode::None,
            Self::HmacSha256 { .. } => AuthenticationMode::HmacSha256,
        }
    }
    pub fn authentication(&self) -> Authentication<'_> {
        match self {
            Self::None {} => Authentication::None,
            Self::HmacSha256 { secret } => Authentication::HmacSha256(secret),
        }
    }
}
