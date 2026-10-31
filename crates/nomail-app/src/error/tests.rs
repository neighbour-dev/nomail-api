use super::*;
use crate::http::ApiFailure;

fn check<E: DomainError + Into<ApiFailure>>(
    error: E,
    domain: &str,
    kind: ErrorKind,
    status: u16,
    message: &str,
) {
    assert_eq!(error.domain(), domain);
    assert_eq!(error.kind(), kind);
    assert_eq!(error.public_message(), message);
    if kind == ErrorKind::Internal {
        let diagnostic = error.diagnostic().unwrap();
        assert_eq!(diagnostic.domain, domain);
        assert_eq!(diagnostic.message, "internal operation failed");
        assert!(!format!("{diagnostic:?}").contains("sensitive"));
    } else {
        assert!(error.diagnostic().is_none());
    }
    let borrowed = ApiFailure::from_domain(&error);
    let owned: ApiFailure = error.into();
    assert_eq!(owned, borrowed);
    assert_eq!(owned.status, status);
    assert_eq!(owned.message, message);
    assert_eq!(
        serde_json::to_value(owned.response()).unwrap(),
        serde_json::json!({"code":status,"msg":message,"data":null})
    );
}

#[test]
fn all_domain_variants_keep_the_existing_wire_contract_and_share_one_policy() {
    use ErrorKind::*;
    check(
        AuthError::InvalidInput("invalid credentials input".into()),
        "auth",
        InvalidInput,
        400,
        "invalid credentials input",
    );
    check(
        AuthError::Unauthorized,
        "auth",
        Unauthorized,
        401,
        "unauthorized",
    );
    check(
        AuthError::NotFound("device not found"),
        "auth",
        NotFound,
        404,
        "device not found",
    );
    check(
        AuthError::Conflict("device limit reached"),
        "auth",
        Conflict,
        409,
        "device limit reached",
    );
    check(
        AuthError::Internal("sensitive token/password".into()),
        "auth",
        Internal,
        500,
        INTERNAL_MESSAGE,
    );
    check(
        MessageError::InvalidInput("invalid message id".into()),
        "message",
        InvalidInput,
        400,
        "invalid message id",
    );
    check(
        MessageError::NotFound("raw payload not found"),
        "message",
        NotFound,
        404,
        "raw payload not found",
    );
    check(
        MessageError::Conflict("resource already exists"),
        "message",
        Conflict,
        409,
        "resource already exists",
    );
    check(
        MessageError::Internal("sensitive raw payload".into()),
        "message",
        Internal,
        500,
        INTERNAL_MESSAGE,
    );
    check(
        ConfigError::InvalidInput("invalid provider".into()),
        "webhook_config",
        InvalidInput,
        400,
        "invalid provider",
    );
    check(
        ConfigError::NotFound,
        "webhook_config",
        NotFound,
        404,
        "webhook config not found",
    );
    check(
        ConfigError::Conflict,
        "webhook_config",
        Conflict,
        409,
        "resource already exists",
    );
    check(
        ConfigError::Internal("sensitive encrypted credential".into()),
        "webhook_config",
        Internal,
        500,
        INTERNAL_MESSAGE,
    );
    check(
        WebhookError::InvalidConfiguration("sensitive secret"),
        "webhook",
        Internal,
        500,
        INTERNAL_MESSAGE,
    );
    check(
        WebhookError::Unauthorized,
        "webhook",
        Unauthorized,
        401,
        "unauthorized",
    );
    check(
        WebhookError::InvalidPayload("invalid payload"),
        "webhook",
        InvalidInput,
        400,
        "invalid payload",
    );
    check(
        WebhookError::Unsupported("unsupported event"),
        "webhook",
        InvalidInput,
        400,
        "unsupported event",
    );
}

#[test]
fn http_only_failures_remain_outside_the_domain_classification() {
    for (status, text) in [
        (413, "request body too large"),
        (429, "too many connections"),
    ] {
        let failure = ApiFailure::new(status, text);
        assert_eq!(failure.status, status);
        assert_eq!(failure.message, text);
    }
}

#[test]
fn old_public_import_paths_are_the_same_types() {
    let _: crate::auth::AuthError = AuthError::Unauthorized;
    let _: crate::message::MessageError = MessageError::NotFound("missing");
    let _: crate::webhook::config::ConfigError = ConfigError::NotFound;
    let _: crate::webhook::WebhookError = WebhookError::Unauthorized;
}
