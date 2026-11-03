use crate::webhook::{Authentication, AuthenticationMode, WebhookError, config::secret};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub enum GitlabCredentials {
    SigningToken {
        #[serde(rename = "signingToken")]
        signing_token: String,
    },
    SecretToken {
        #[serde(rename = "secretToken")]
        secret_token: String,
    },
}
impl GitlabCredentials {
    pub fn validate(&self) -> Result<(), WebhookError> {
        match self {
            Self::SecretToken { secret_token } => secret(secret_token),
            Self::SigningToken { signing_token } => {
                secret(signing_token)?;
                if signing_token
                    .strip_prefix("whsec_")
                    .and_then(|s| STANDARD.decode(s).ok())
                    .is_none_or(|key| key.is_empty())
                {
                    return Err(WebhookError::InvalidConfiguration(
                        "invalid GitLab signing token",
                    ));
                }
                Ok(())
            }
        }
    }
    pub fn mode(&self) -> AuthenticationMode {
        match self {
            Self::SigningToken { .. } => AuthenticationMode::GitlabSigningToken,
            Self::SecretToken { .. } => AuthenticationMode::GitlabSecretToken,
        }
    }
    pub fn authentication(&self) -> Authentication<'_> {
        match self {
            Self::SigningToken { signing_token } => {
                Authentication::GitlabSigningToken(signing_token)
            }
            Self::SecretToken { secret_token } => Authentication::GitlabToken(secret_token),
        }
    }
}
