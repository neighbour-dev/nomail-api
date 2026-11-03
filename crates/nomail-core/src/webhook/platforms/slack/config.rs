use crate::webhook::{Authentication, AuthenticationMode, WebhookError, config::secret};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct SlackCredentials {
    pub signing_secret: String,
}
impl SlackCredentials {
    pub fn validate(&self) -> Result<(), WebhookError> {
        secret(&self.signing_secret)
    }
    pub fn mode(&self) -> AuthenticationMode {
        AuthenticationMode::SlackSigningSecret
    }
    pub fn authentication(&self) -> Authentication<'_> {
        Authentication::SlackSigningSecret(&self.signing_secret)
    }
}
