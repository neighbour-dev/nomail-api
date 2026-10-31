use super::*;
use crate::{message::*, webhook::MessageLevel};
use std::cell::{Cell, RefCell};

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
struct Store {
    provider: Provider,
    secret: Option<String>,
    row: RefCell<Option<Message>>,
    fail_write: bool,
}
impl IngestionStore for Store {
    async fn receiving_config(&self, key: &str) -> IngestionResult<Option<ReceivingConfig>> {
        Ok((key == "enabled").then(|| ReceivingConfig {
            id: 12,
            user_id: 7,
            provider: self.provider,
            source: Some("Configured source".into()),
            encrypted_secret: self.secret.clone(),
        }))
    }
}
impl MessageStore for Store {
    async fn list_owned(
        &self,
        _: i64,
        _: &MessageQuery,
    ) -> MessageResult<(Vec<MessageListItem>, u64)> {
        unreachable!()
    }
    async fn count_unread_owned(&self, _: i64) -> MessageResult<u64> {
        unreachable!()
    }
    async fn get_owned(&self, _: i64, _: Uuid) -> MessageResult<Option<MessageWithConfig>> {
        unreachable!()
    }
    async fn get_raw_owned(&self, _: i64, _: Uuid) -> MessageResult<Option<RawContent>> {
        unreachable!()
    }
    async fn update_owned(
        &self,
        _: i64,
        _: Uuid,
        _: &StatePatch,
    ) -> MessageResult<Option<MessageWithConfig>> {
        unreachable!()
    }
    async fn mutate_many_owned(
        &self,
        _: i64,
        _: &[Uuid],
        _: &MessageMutation,
    ) -> MessageResult<u64> {
        unreachable!()
    }
    async fn insert_once(&self, message: &Message) -> MessageResult<InsertOutcome> {
        if self.fail_write {
            return Err(MessageError::Internal("write failed".into()));
        }
        let mut row = self.row.borrow_mut();
        if let Some(previous) = row.as_ref()
            && previous.metadata.deduplication_key == message.metadata.deduplication_key
        {
            return Ok(InsertOutcome::Duplicate(previous.id));
        }
        *row = Some(message.clone());
        Ok(InsertOutcome::Created(message.id))
    }
}
fn service(provider: Provider, secret: Option<&str>) -> IngestionService<Store, Runtime> {
    IngestionService {
        messages: MessageService {
            store: Store {
                provider,
                secret: secret
                    .map(|value| crate::security::encrypt(value, &[7; 32], &[3; 12]).unwrap()),
                row: RefCell::new(None),
                fail_write: false,
            },
        },
        runtime: Runtime(Cell::new(0)),
    }
}

#[tokio::test]
async fn application_owns_acceptance_defaults_and_duplicate_effects() {
    let service = service(Provider::Generic, None);
    let headers = vec![];
    let request = WebhookRequest {
        headers: &headers,
        body: br#"{"id":"event-1","title":" Alert ","content":" body ","level":"high"}"#,
    };
    let IngestionOutcome::Accepted {
        user_id,
        acceptance,
    } = service.receive("enabled", &request).await.unwrap()
    else {
        panic!()
    };
    assert_eq!(user_id, 7);
    assert_eq!(
        acceptance.outcome,
        InsertOutcome::Created(Uuid::from_u128(1))
    );
    let event = acceptance.created_event.unwrap();
    assert_eq!(event.title, "Alert");
    assert_eq!(event.source, "Configured source");
    assert_eq!(event.level, MessageLevel::High);
    let row = service.messages.store.row.borrow().clone().unwrap();
    assert_eq!(row.webhook_config_id, 12);
    assert_eq!(row.status, "unread");
    assert_eq!(
        row.expires_at,
        Some(row.received_at + chrono::Duration::days(3))
    );
    let IngestionOutcome::Accepted { acceptance, .. } =
        service.receive("enabled", &request).await.unwrap()
    else {
        panic!()
    };
    assert_eq!(acceptance.outcome, InsertOutcome::Duplicate(row.id));
    assert!(acceptance.created_event.is_none());
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
    assert!(service.messages.store.row.borrow().is_none());
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
    service.messages.store.secret = Some("invalid ciphertext".into());
    assert!(matches!(
        service.receive("enabled", &request).await,
        Err(IngestionError::Internal(_))
    ));
    service.messages.store.secret = None;
    service.messages.store.fail_write = true;
    assert!(matches!(
        service.receive("enabled", &request).await,
        Err(IngestionError::Message(_))
    ));
    assert!(service.messages.store.row.borrow().is_none());
}
