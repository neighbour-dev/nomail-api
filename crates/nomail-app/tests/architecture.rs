//! Guard the module boundary inside the application crate. The HTTP contract can
//! depend on use cases; production use cases must not import HTTP presentation.
use std::path::Path;

#[test]
fn deployments_do_not_bind_or_parse_individual_endpoints() {
    fn check(path: &Path) {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                check(&path);
            } else if path.extension().is_some_and(|v| v == "rs") {
                let source = std::fs::read_to_string(&path).unwrap();
                for forbidden in [
                    "Operation::",
                    "endpoints as",
                    "parse_json::<",
                    "parse_query::<",
                    "success::<",
                ] {
                    assert!(
                        !source.contains(forbidden),
                        "{} repeats HTTP dispatch through {forbidden}",
                        path.display()
                    );
                }
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    check(&root.join("nomail-server/src"));
    check(&root.join("nomail-workers/src"));
}

#[test]
fn business_modules_do_not_depend_on_http() {
    fn check(path: &Path) {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                check(&path);
            } else if path.extension().is_some_and(|v| v == "rs")
                && path.file_name().is_some_and(|v| v != "tests.rs")
            {
                let source = std::fs::read_to_string(&path).unwrap();
                for forbidden in [
                    "http::",
                    "api::",
                    "actix_web",
                    "worker::",
                    "sea_orm",
                    "sqlx::",
                    "schemars",
                    "JsonSchema",
                    "LoginRequest",
                    "TokenResponse",
                    "AccountResponse",
                    "DeviceResponse",
                    "MessageListItem",
                    "MessageDetail",
                    "PaginationQuery",
                    "CountResponse",
                    "AffectedResponse",
                ] {
                    assert!(
                        !source.contains(forbidden),
                        "{} depends on {forbidden}",
                        path.display()
                    );
                }
            }
        }
    }
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for module in ["auth", "message", "webhook", "ingestion"] {
        check(&src.join(module));
    }
    let pagination = std::fs::read_to_string(src.join("pagination.rs")).unwrap();
    for forbidden in ["serde", "schemars", "Response", "PaginationQuery"] {
        assert!(
            !pagination.contains(forbidden),
            "business pagination contains {forbidden}"
        );
    }
}

#[test]
fn application_wire_schemas_are_defined_only_in_the_contract_directory() {
    fn check(path: &Path) {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                if path.file_name().is_some_and(|name| name != "contract") {
                    check(&path);
                }
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let source = std::fs::read_to_string(&path).unwrap();
                assert!(
                    !source.contains("derive(schemars::JsonSchema)"),
                    "{} defines a wire schema outside contract",
                    path.display()
                );
            }
        }
    }
    check(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"));
}

#[test]
fn protocol_crate_has_no_community_or_deployment_dependencies() {
    let core = Path::new(env!("CARGO_MANIFEST_DIR")).join("../nomail-core");
    let manifest = std::fs::read_to_string(core.join("Cargo.toml")).unwrap();
    for dependency in [
        "nomail-app",
        "nomail-server",
        "nomail-workers",
        "bcrypt",
        "jsonwebtoken",
        "sea-orm",
        "actix-web",
    ] {
        assert!(
            !manifest.contains(dependency),
            "protocol crate depends on {dependency}"
        );
    }
    let lib = std::fs::read_to_string(core.join("src/lib.rs")).unwrap();
    for module in ["auth", "api", "http", "message", "security"] {
        assert!(!lib.contains(&format!("pub mod {module};")));
    }
}

#[test]
fn workers_configuration_and_randomness_do_not_use_http_errors() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../nomail-workers/src");
    for name in ["config.rs", "crypto.rs"] {
        let source = std::fs::read_to_string(src.join(name)).unwrap();
        for forbidden in ["http::", "ApiError", "ApiFailure", "crate::error"] {
            assert!(
                !source.contains(forbidden),
                "Workers {name} depends on {forbidden}"
            );
        }
    }
    let adapter = std::fs::read_to_string(src.join("error.rs")).unwrap();
    for forbidden in [
        "DomainError",
        "from_domain",
        "UNIQUE constraint failed",
        "into_http",
        "AuthError",
        "MessageError",
        "ConfigError",
        "IngestionError",
        "WebhookError",
    ] {
        assert!(
            !adapter.contains(forbidden),
            "Workers response adapter handles {forbidden}"
        );
    }
    let services = std::fs::read_to_string(src.join("services.rs")).unwrap();
    assert!(!services.contains("into_http"));
}
