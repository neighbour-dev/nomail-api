use crate::webhook::{Authentication, AuthenticationMode, WebhookError, config::secret};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct FeishuCredentials {
    pub verification_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypt_key: Option<String>,
}
impl FeishuCredentials {
    pub fn validate(&self) -> Result<(), WebhookError> {
        secret(&self.verification_token)?;
        if let Some(key) = &self.encrypt_key {
            secret(key)?;
        }
        Ok(())
    }
    pub fn mode(&self) -> AuthenticationMode {
        if self.encrypt_key.is_some() {
            AuthenticationMode::FeishuEncrypted
        } else {
            AuthenticationMode::FeishuToken
        }
    }
    pub fn authentication(&self) -> Authentication<'_> {
        Authentication::Feishu {
            verification_token: Some(&self.verification_token),
            encrypt_key: self.encrypt_key.as_deref(),
        }
    }
}
