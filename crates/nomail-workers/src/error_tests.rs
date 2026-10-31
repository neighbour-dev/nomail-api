use super::*;
use nomail_core::error::{AuthError, ConfigError, MessageError, WebhookError};

fn check<E: DomainError + Into<ApiError>>(error: E) {
    let expected = ApiFailure::from_domain(&error);
    let diagnostic = error.diagnostic();
    let actual: ApiError = error.into();
    assert_eq!(actual.status, expected.status);
    assert_eq!(actual.message, expected.message);
    assert_eq!(actual.diagnostic, diagnostic);
}

#[test]
fn adapters_delegate_all_domains_without_invoking_runtime_logging_during_conversion() {
    // Runs natively: conversion must not call the WASM console or construct a
    // Workers Response. Both are deferred to the final runtime boundary.
    check(AuthError::Unauthorized);
    check(AuthError::Internal("sensitive password".into()));
    check(MessageError::InvalidInput("invalid message id".into()));
    check(MessageError::NotFound("message not found"));
    check(MessageError::Internal("sensitive raw payload".into()));
    check(ConfigError::Conflict);
    check(ConfigError::Internal(
        "sensitive encrypted credential".into(),
    ));
    check(WebhookError::Unsupported("unsupported event"));
    check(WebhookError::InvalidConfiguration("sensitive secret"));
    let storage: ApiError = worker::Error::RustError("sensitive database value".into()).into();
    assert_eq!(storage.message, "internal server error");
    assert!(!format!("{storage:?}").contains("sensitive"));
}
