use super::*;
use nomail_app::error::{AuthError, ConfigError, DomainError, MessageError, WebhookError};
use nomail_app::ingestion::IngestionError;

fn check<E: DomainError + Into<HttpError>>(error: E) {
    let expected = ApiFailure::from_domain(&error);
    let diagnostic = error.diagnostic();
    let actual: ApiError = error.into().into();
    assert_eq!(actual.status_code().as_u16(), expected.status);
    assert_eq!(actual.to_string(), expected.message);
    assert_eq!(actual.0.diagnostic, diagnostic);
}

#[test]
fn adapter_preserves_shared_http_errors_without_mapping_domains_itself() {
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
    check(IngestionError::Internal("sensitive database value".into()));
    let domain = MessageError::Internal("sensitive raw payload".into());
    let diagnostic = domain.diagnostic();
    let shared: nomail_app::http::HttpError = domain.into();
    let actual: ApiError = shared.into();
    assert_eq!(actual.0.diagnostic, diagnostic);
    assert_eq!(actual.to_string(), "internal server error");
    assert!(!format!("{actual:?}").contains("sensitive"));
}

#[actix_web::test]
async fn actix_status_and_json_envelope_match_shared_failures() {
    for status in [400, 401, 404, 409, 413, 415, 429, 500] {
        let failure = ApiFailure::new(status, "safe public message");
        let expected = failure.clone().response();
        let error: ApiError = failure.into();
        let response = error.error_response();
        assert_eq!(response.status().as_u16(), status);
        let body = actix_web::body::to_bytes(response.into_body())
            .await
            .unwrap();
        let actual: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(actual, serde_json::to_value(expected).unwrap());
    }
    let error: ApiError = ApiFailure::new(0, "safe public message").into();
    assert_eq!(error.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
}
