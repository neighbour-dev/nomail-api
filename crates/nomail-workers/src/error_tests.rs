use super::*;
use nomail_app::error::{AuthError, ConfigError, DomainError, MessageError, WebhookError};

fn check<E: DomainError + Into<HttpError>>(error: E) {
    let expected = ApiFailure::from_domain(&error);
    let diagnostic = error.diagnostic();
    let actual: ApiError = error.into().into();
    assert_eq!(actual.0.failure, expected);
    assert_eq!(actual.0.diagnostic, diagnostic);
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
    assert_eq!(storage.0.failure.message, "internal server error");
    assert!(!format!("{storage:?}").contains("sensitive"));
    let domain = MessageError::Internal("sensitive raw payload".into());
    let diagnostic = domain.diagnostic();
    let shared: nomail_app::http::HttpError = domain.into();
    let actual: ApiError = shared.into();
    assert_eq!(actual.0.diagnostic, diagnostic);
    assert_eq!(actual.0.failure.message, "internal server error");
    assert!(!format!("{actual:?}").contains("sensitive"));
}

#[test]
fn platform_failures_do_not_guess_database_conflicts() {
    let error: ApiError =
        worker::Error::RustError("UNIQUE constraint failed: sensitive value".into()).into();
    assert_eq!(error.0.failure, ApiFailure::internal());
    assert!(!format!("{error:?}").contains("sensitive"));
    let error: ApiError = HttpError::from(ConfigError::Conflict).into();
    assert_eq!(error.0.failure.status, 409);
}
