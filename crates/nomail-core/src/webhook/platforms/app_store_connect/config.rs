use crate::webhook::{Authentication, AuthenticationMode, WebhookError, config::secret};
use serde::{Deserialize, Serialize};

/// The webhook Secret, not an App Store Connect JWT/private key or IAP secret.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct AppStoreConnectCredentials {
    pub secret: String,
}

impl AppStoreConnectCredentials {
    pub fn validate(&self) -> Result<(), WebhookError> {
        secret(&self.secret)
    }
    pub fn mode(&self) -> AuthenticationMode {
        AuthenticationMode::HmacSha256
    }
    pub fn authentication(&self) -> Authentication<'_> {
        Authentication::HmacSha256(&self.secret)
    }
}
