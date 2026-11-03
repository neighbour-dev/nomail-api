use crate::webhook::{Authentication, AuthenticationMode, WebhookError, config::secret};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct StripeCredentials {
    pub signing_secrets: Vec<String>,
}
impl StripeCredentials {
    pub fn validate(&self) -> Result<(), WebhookError> {
        // Receiver resource bound, not a Stripe platform limit.
        if self.signing_secrets.is_empty() || self.signing_secrets.len() > 5 {
            return Err(WebhookError::InvalidConfiguration(
                "configure 1 to 5 Stripe signing secrets",
            ));
        }
        for value in &self.signing_secrets {
            secret(value)?;
            if !value.starts_with("whsec_") || value.len() <= 6 {
                return Err(WebhookError::InvalidConfiguration(
                    "invalid Stripe signing secret",
                ));
            }
        }
        Ok(())
    }
    pub fn mode(&self) -> AuthenticationMode {
        AuthenticationMode::StripeSigningSecrets
    }
    pub fn authentication(&self) -> Authentication<'_> {
        Authentication::StripeSigningSecrets(&self.signing_secrets)
    }
}
