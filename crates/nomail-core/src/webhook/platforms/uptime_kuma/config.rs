use crate::webhook::{Authentication, AuthenticationMode, WebhookError};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct UptimeKumaCredentials {}

impl UptimeKumaCredentials {
    pub fn validate(&self) -> Result<(), WebhookError> {
        Ok(())
    }

    pub fn mode(&self) -> AuthenticationMode {
        AuthenticationMode::None
    }

    pub fn authentication(&self) -> Authentication<'_> {
        Authentication::None
    }
}
