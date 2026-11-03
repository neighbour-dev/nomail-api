use super::*;
use crate::webhook::MessageLevel;
use serde_json::json;
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};

struct Runtime(Cell<u128>);
impl IngestionRuntime for Runtime {
    fn now(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(1000, 0).unwrap()
    }
    fn message_id(&self, _: DateTime<Utc>) -> IngestionResult<Uuid> {
        self.0.set(self.0.get() + 1);
        Ok(Uuid::from_u128(self.0.get()))
    }
    fn encryption_key(&self) -> &[u8; 32] {
        &[7; 32]
    }
}

// Reception deliberately has no MessageStore implementation, no message
// management stubs, and no Clone requirement.
struct Store {
    provider: Provider,
    secret: Option<String>,
    rows: RefCell<HashMap<Uuid, Message>>,
    fail_write: bool,
    forced_outcome: Option<InsertOutcome>,
}
impl IngestionStore for Store {
    async fn receiving_config(&self, key: &str) -> IngestionResult<Option<ReceivingConfig>> {
        let id = match key {
            "enabled" => 12,
            "other" => 13,
            _ => return Ok(None),
        };
        Ok(Some(ReceivingConfig {
            id,
            user_id: 7,
            provider: self.provider,
            encrypted_platform: self.secret.clone().unwrap(),
        }))
    }
    async fn insert_once(&self, message: &Message) -> MessageResult<InsertOutcome> {
        if self.fail_write {
            return Err(MessageError::Internal("write failed".into()));
        }
        if let Some(outcome) = self.forced_outcome {
            return Ok(outcome);
        }
        let mut rows = self.rows.borrow_mut();
        if let Some(key) = message.metadata.deduplication_key.as_ref()
            && let Some(previous) = rows.values().find(|previous| {
                previous.webhook_config_id == message.webhook_config_id
                    && previous.metadata.deduplication_key.as_ref() == Some(key)
            })
        {
            return Ok(InsertOutcome::Duplicate(previous.id));
        }
        if rows.contains_key(&message.id) {
            return Err(MessageError::Conflict("duplicate id"));
        }
        rows.insert(message.id, message.clone());
        Ok(InsertOutcome::Created(message.id))
    }
}
fn service(provider: Provider, secret: Option<&str>) -> IngestionService<Store, Runtime> {
    IngestionService {
        store: Store {
            provider,
            secret: Some(encrypted(provider, secret)),
            rows: RefCell::new(HashMap::new()),
            fail_write: false,
            forced_outcome: None,
        },
        runtime: Runtime(Cell::new(0)),
    }
}
fn encrypted(provider: Provider, secret: Option<&str>) -> String {
    let platform = match provider {
        Provider::Generic => json!({"provider":"generic","credentials":match secret {
            Some(s) => json!({"type":"hmacSha256","secret":s}),
            None => json!({"type":"none"}),
        }}),
        Provider::Github => {
            json!({"provider":"github","credentials":{"secret":secret.unwrap_or("secret")}})
        }
        Provider::Feishu => {
            json!({"provider":"feishu","credentials":{"verificationToken":secret.unwrap(),"encryptKey":"separate-encryption-key"}})
        }
        _ => panic!("unexpected fixture provider"),
    };
    crate::security::encrypt(&platform.to_string(), &[7; 32], &[3; 12]).unwrap()
}
fn accepted(outcome: IngestionOutcome) -> Acceptance {
    let IngestionOutcome::Accepted {
        user_id,
        acceptance,
    } = outcome
    else {
        panic!("expected a business delivery")
    };
    assert_eq!(user_id, 7);
    acceptance
}
async fn receive_json(
    service: &IngestionService<Store, Runtime>,
    key: &str,
    body: serde_json::Value,
) -> Acceptance {
    let headers = vec![];
    let body = serde_json::to_vec(&body).unwrap();
    accepted(
        service
            .receive(
                key,
                &WebhookRequest {
                    headers: &headers,
                    body: &body,
                },
            )
            .await
            .unwrap(),
    )
}

#[tokio::test]
async fn application_owns_acceptance_defaults_and_duplicate_effects() {
    let service = service(Provider::Generic, None);
    let request = json!({"id":"event-1","title":" Alert ","content":" body ","level":"high"});
    let acceptance = receive_json(&service, "enabled", request.clone()).await;
    assert_eq!(
        acceptance.outcome,
        InsertOutcome::Created(Uuid::from_u128(1))
    );
    let event = acceptance.created_event.unwrap();
    assert_eq!(event.title, "Alert");
    assert_eq!(event.source, "generic");
    assert_eq!(event.level, MessageLevel::High);
    assert_eq!(event.status, "unread");
    assert!(!event.pinned);
    let row = service.store.rows.borrow()[&event.id].clone();
    assert_eq!(row.webhook_config_id, 12);
    assert_eq!(row.common.content, "body");
    assert_eq!(row.status, "unread");
    assert_eq!(
        row.expires_at,
        Some(row.received_at + chrono::Duration::days(3))
    );
    assert_eq!(event.expires_at, row.expires_at);
    let duplicate = receive_json(&service, "enabled", request).await;
    assert_eq!(duplicate.outcome, InsertOutcome::Duplicate(row.id));
    assert!(duplicate.created_event.is_none());
    assert_eq!(service.store.rows.borrow().len(), 1);
}

#[tokio::test]
async fn duplicate_deliveries_after_soft_deletion_do_not_rewrite_or_emit() {
    let service = service(Provider::Generic, None);
    let first = receive_json(&service, "enabled", json!({"id":"same","title":"Original"})).await;
    let id = first.outcome.id();
    let deleted_at = service.runtime.now() + chrono::Duration::seconds(1);
    service
        .store
        .rows
        .borrow_mut()
        .get_mut(&id)
        .unwrap()
        .deleted_at = Some(deleted_at);
    let duplicate = receive_json(&service, "enabled", json!({"id":"same","title":"Changed"})).await;
    assert_eq!(duplicate.outcome, InsertOutcome::Duplicate(id));
    assert!(duplicate.created_event.is_none());
    let rows = service.store.rows.borrow();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[&id].common.title, "Original");
    assert_eq!(rows[&id].deleted_at, Some(deleted_at));
}

#[tokio::test]
async fn deduplication_keeps_configuration_scopes_full_keys_and_null_key_independence() {
    let mut service = service(Provider::Generic, None);
    let key = "x".repeat(400);
    let request = json!({"id":key,"title":"Long key"});
    let first = receive_json(&service, "enabled", request.clone()).await;
    let duplicate = receive_json(&service, "enabled", request.clone()).await;
    assert_eq!(
        duplicate.outcome,
        InsertOutcome::Duplicate(first.outcome.id())
    );
    assert!(duplicate.created_event.is_none());
    let other_config = receive_json(&service, "other", request).await;
    assert!(!other_config.outcome.duplicate());
    assert_ne!(other_config.outcome.id(), first.outcome.id());
    let other_key = receive_json(
        &service,
        "enabled",
        json!({"id":format!("{key}y"),"title":"Different suffix"}),
    )
    .await;
    assert!(!other_key.outcome.duplicate());

    // Reconfiguring a source must not collapse equal identifiers from different
    // protocol scopes into the same delivery.
    service.store.provider = Provider::Github;
    service.store.secret = Some(encrypted(Provider::Github, Some("secret")));
    let headers = vec![
        ("X-GitHub-Event".into(), "issues".into()),
        ("X-GitHub-Delivery".into(), key.clone()),
        (
            "X-Hub-Signature-256".into(),
            "sha256=02849cb8a3f27f99a4aa460476e3715fa5104b10acfbf6c2d80e3e3a9bfa8c7f".into(),
        ),
    ];
    let github = accepted(
        service
            .receive(
                "enabled",
                &WebhookRequest {
                    headers: &headers,
                    body: br#"{"action":"opened","issue":{"title":"Issue"}}"#,
                },
            )
            .await
            .unwrap(),
    );
    assert!(!github.outcome.duplicate());
    {
        let rows = service.store.rows.borrow();
        let generic_key = rows[&first.outcome.id()]
            .metadata
            .deduplication_key
            .as_ref()
            .unwrap();
        let github_key = rows[&github.outcome.id()]
            .metadata
            .deduplication_key
            .as_ref()
            .unwrap();
        assert_eq!(generic_key.value, key);
        assert_eq!(github_key.value, key);
        assert_ne!(generic_key.scope, github_key.scope);
    }

    service.store.provider = Provider::Generic;
    service.store.secret = Some(encrypted(Provider::Generic, None));
    for _ in 0..2 {
        let acceptance = receive_json(&service, "enabled", json!({"title":"No delivery id"})).await;
        assert!(!acceptance.outcome.duplicate());
        assert!(acceptance.created_event.is_some());
    }
    assert_eq!(service.store.rows.borrow().len(), 6);
}

#[tokio::test]
async fn controls_are_authenticated_without_creating_identity_or_writing() {
    let service = service(Provider::Feishu, Some("verification-token"));
    let headers = vec![];
    let request = WebhookRequest {
        headers: &headers,
        body: br#"{"type":"url_verification","token":"verification-token","challenge":"hello"}"#,
    };
    let IngestionOutcome::Control(response) = service.receive("enabled", &request).await.unwrap()
    else {
        panic!()
    };
    assert_eq!(response.status, 200);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&response.body).unwrap()["challenge"],
        "hello"
    );
    assert_eq!(service.runtime.0.get(), 0);
    assert!(service.store.rows.borrow().is_empty());
    let bad = WebhookRequest {
        headers: &headers,
        body: br#"{"type":"url_verification","token":"wrong","challenge":"hello"}"#,
    };
    assert!(matches!(
        service.receive("enabled", &bad).await,
        Err(IngestionError::Protocol(_))
    ));
    let unsigned_event = WebhookRequest {
        headers: &headers,
        body: br#"{"type":"event","token":"verification-token","event":{}}"#,
    };
    assert!(matches!(
        service.receive("enabled", &unsigned_event).await,
        Err(IngestionError::Protocol(_))
    ));
    assert_eq!(service.runtime.0.get(), 0);
}

#[tokio::test]
async fn missing_invalid_credentials_and_failed_writes_never_acknowledge() {
    let mut service = service(Provider::Generic, Some("secret"));
    let headers = vec![];
    let request = WebhookRequest {
        headers: &headers,
        body: br#"{"id":"one","title":"Alert"}"#,
    };
    assert!(matches!(
        service.receive("disabled", &request).await,
        Err(IngestionError::ConfigNotFound)
    ));
    assert!(matches!(
        service.receive("enabled", &request).await,
        Err(IngestionError::Protocol(_))
    ));
    assert_eq!(service.runtime.0.get(), 0);
    service.store.secret = Some("invalid ciphertext".into());
    assert!(matches!(
        service.receive("enabled", &request).await,
        Err(IngestionError::Internal(_))
    ));
    service.store.secret = Some(encrypted(Provider::Generic, None));
    service.store.fail_write = true;
    assert!(matches!(
        service.receive("enabled", &request).await,
        Err(IngestionError::Message(_))
    ));
    assert!(service.store.rows.borrow().is_empty());
}

#[tokio::test]
async fn inconsistent_created_id_never_produces_an_acknowledgement() {
    let mut service = service(Provider::Generic, None);
    service.store.forced_outcome = Some(InsertOutcome::Created(Uuid::from_u128(999)));
    let headers = vec![];
    let request = WebhookRequest {
        headers: &headers,
        body: br#"{"id":"one","title":"Alert"}"#,
    };
    assert!(matches!(service.receive("enabled", &request).await,
        Err(IngestionError::Message(MessageError::Internal(message)))
            if message == "insert returned an inconsistent message id"));
}
