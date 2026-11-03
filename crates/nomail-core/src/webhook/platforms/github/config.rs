use crate::webhook::{Authentication, AuthenticationMode, WebhookError, config::secret};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct GithubCredentials {
    pub secret: String,
}
impl GithubCredentials {
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
