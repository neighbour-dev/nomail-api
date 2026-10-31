//! Transport-neutral application error policy, without logging or HTTP statuses.
pub const INTERNAL_MESSAGE: &str = "internal server error";
pub const UNAUTHORIZED_MESSAGE: &str = "unauthorized";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    InvalidInput,
    Unauthorized,
    NotFound,
    Conflict,
    Internal,
}

/// Safe operational context, deliberately limited to static labels. Arbitrary
/// Internal(String)/Display text is NOT a log message: it can contain credentials,
/// database values or upstream payloads. Detailed debugging needs explicit,
/// source-specific sanitization rather than automatic forwarding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub domain: &'static str,
    pub message: &'static str,
}

pub trait DomainError: std::error::Error {
    fn kind(&self) -> ErrorKind;
    fn domain(&self) -> &'static str;
    /// Safe client-facing description; never return internal diagnostic text.
    fn public_message(&self) -> &str;
    /// Only internal failures are logged by the runtime boundary. Expected
    /// validation/authentication failures are not application error logs.
    fn diagnostic(&self) -> Option<Diagnostic> {
        (self.kind() == ErrorKind::Internal).then(|| Diagnostic {
            domain: self.domain(),
            message: "internal operation failed",
        })
    }
}
