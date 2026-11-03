use crate::webhook::{Authentication, AuthenticationMode, WebhookError};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct TelegramCredentials {
    pub secret_token: String,
}
impl TelegramCredentials {
    pub fn validate(&self) -> Result<(), WebhookError> {
        if self.secret_token.is_empty()
            || self.secret_token.len() > 256
            || !self
                .secret_token
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err(WebhookError::InvalidConfiguration(
                "invalid Telegram secret token",
            ));
        }
        Ok(())
    }
    pub fn mode(&self) -> AuthenticationMode {
        AuthenticationMode::TelegramSecretToken
    }
    pub fn authentication(&self) -> Authentication<'_> {
        Authentication::TelegramSecretToken(&self.secret_token)
    }
}
