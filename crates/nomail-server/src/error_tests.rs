use super::*;
use nomail_app::error::{AuthError, ConfigError, MessageError, WebhookError};
use std::{
    io::Write,
    sync::{Arc, Mutex},
};

fn check<E: DomainError + Into<ApiError>>(error: E) {
    let expected = ApiFailure::from_domain(&error);
    let diagnostic = error.diagnostic();
    let actual: ApiError = error.into();
    assert_eq!(actual.status.as_u16(), expected.status);
    assert_eq!(actual.message, expected.message);
    assert_eq!(actual.diagnostic, diagnostic);
}

#[test]
fn adapters_delegate_all_domains_to_the_shared_mapping() {
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
    let storage: ApiError = sea_orm::DbErr::Custom("sensitive database value".into()).into();
    assert_eq!(storage.message, "internal server error");
    assert!(!format!("{storage:?}").contains("sensitive"));
}

#[derive(Clone)]
struct Writer(Arc<Mutex<Vec<u8>>>);
impl Write for Writer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn conversions_are_silent_and_response_logs_once_without_raw_internal_text() {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let output = bytes.clone();
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer(move || Writer(output.clone()))
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        let storage = ApiError::internal("sensitive database value");
        let domain = AuthError::Internal(storage.message);
        let error: ApiError = domain.into();
        assert!(bytes.lock().unwrap().is_empty());
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let expected: ApiError = AuthError::Unauthorized.into();
        assert_eq!(expected.error_response().status(), StatusCode::UNAUTHORIZED);
    });
    let text = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
    assert_eq!(text.matches("request failed").count(), 1);
    assert!(text.contains("auth"));
    assert!(!text.contains("sensitive"));
}
