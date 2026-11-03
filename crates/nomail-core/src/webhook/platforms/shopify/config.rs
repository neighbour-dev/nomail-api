use crate::webhook::{Authentication, AuthenticationMode, WebhookError, config::secret};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct ShopifyCredentials {
    pub client_secret: String,
}
impl ShopifyCredentials {
    pub fn validate(&self) -> Result<(), WebhookError> {
        secret(&self.client_secret)
    }
    pub fn mode(&self) -> AuthenticationMode {
        AuthenticationMode::ShopifyClientSecret
    }
    pub fn authentication(&self) -> Authentication<'_> {
        Authentication::ShopifyClientSecret(&self.client_secret)
    }
}
