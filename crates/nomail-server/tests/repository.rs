use std::path::Path;

#[test]
fn actix_errors_do_not_escape_the_transport_adapter() {
    fn check(directory: &Path) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                check(&path);
            } else if path.extension().is_some_and(|extension| extension == "rs")
                && !["app.rs", "error.rs", "error_tests.rs"]
                    .contains(&path.file_name().unwrap().to_str().unwrap())
            {
                let source = std::fs::read_to_string(&path).unwrap();
                for forbidden in ["ApiError", "ApiResult"] {
                    assert!(
                        !source.contains(forbidden),
                        "{} uses HTTP error {forbidden} outside the adapter",
                        path.display()
                    );
                }
            }
        }
    }
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    check(&src);
    let adapter = std::fs::read_to_string(src.join("error.rs")).unwrap();
    for forbidden in [
        "DomainError",
        "from_domain",
        "DbErr",
        "AuthError",
        "MessageError",
        "ConfigError",
        "WebhookError",
        "IngestionError",
    ] {
        assert!(
            !adapter.contains(forbidden),
            "Actix adapter maps {forbidden} instead of shared HTTP errors"
        );
    }
}

#[test]
fn repository_has_no_go_sources_or_build_manifests() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    fn check(directory: &Path) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                if ![
                    ".git",
                    "target",
                    ".idea",
                    "node_modules",
                    ".wrangler",
                    "build",
                ]
                .contains(&path.file_name().unwrap().to_str().unwrap_or(""))
                {
                    check(&path);
                }
            } else {
                assert_ne!(
                    path.extension().and_then(|v| v.to_str()),
                    Some("go"),
                    "unexpected Go source: {}",
                    path.display()
                );
            }
        }
    }
    check(&root);
    for name in ["go.mod", "go.sum", "go.work", "nomail-api-linux-amd64"] {
        assert!(!root.join(name).exists(), "legacy artifact remains: {name}");
    }
}

#[test]
fn openapi_covers_the_core_contract_and_valid_local_references() {
    let document: serde_json::Value =
        serde_json::from_str(include_str!("../../../docs/swagger.json")).unwrap();
    let operations = document["paths"]
        .as_object()
        .unwrap()
        .values()
        .map(|path| {
            path.as_object()
                .unwrap()
                .keys()
                .filter(|method| {
                    ["get", "post", "patch", "delete", "put"].contains(&method.as_str())
                })
                .count()
        })
        .sum::<usize>();
    assert_eq!(operations, nomail_app::http::ROUTES.len());
    assert!(
        !document["paths"]
            .as_object()
            .unwrap()
            .keys()
            .any(|path| path.starts_with("/api/push"))
    );
    fn references(value: &serde_json::Value, root: &serde_json::Value) {
        match value {
            serde_json::Value::Object(object) => {
                if let Some(reference) = object
                    .get("$ref")
                    .and_then(|v| v.as_str())
                    .and_then(|v| v.strip_prefix('#'))
                {
                    assert!(
                        root.pointer(reference).is_some(),
                        "invalid reference: {reference}"
                    );
                }
                for value in object.values() {
                    references(value, root);
                }
            }
            serde_json::Value::Array(array) => {
                for value in array {
                    references(value, root);
                }
            }
            _ => (),
        }
    }
    references(&document, &document);
}
