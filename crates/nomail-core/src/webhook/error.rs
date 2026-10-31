use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebhookError {
    InvalidConfiguration(&'static str),
    Unauthorized,
    InvalidPayload(&'static str),
    Unsupported(&'static str),
}
impl fmt::Display for WebhookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidConfiguration(message)
            | Self::InvalidPayload(message)
            | Self::Unsupported(message) => message,
            Self::Unauthorized => "unauthorized",
        })
    }
}
impl std::error::Error for WebhookError {}
