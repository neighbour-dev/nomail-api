use super::*;
use crate::{pagination::PaginationQuery, webhook::Provider};
use chrono::{DateTime, Utc};
use std::cell::{Cell, RefCell};

fn now() -> DateTime<Utc> {
    DateTime::from_timestamp(1000, 0).unwrap()
}
struct Runtime {
    random_calls: Cell<u8>,
    fail: bool,
}
impl WebhookConfigRuntime for Runtime {
    fn now(&self) -> DateTime<Utc> {
        now()
    }
    fn random_bytes(&self, bytes: &mut [u8]) -> ConfigResult<()> {
        if self.fail {
            return Err(ConfigError::Internal("random unavailable".into()));
        }
        self.random_calls.set(self.random_calls.get() + 1);
        bytes.fill(self.random_calls.get());
        Ok(())
    }
    fn encryption_key(&self) -> &[u8; 32] {
        &[9; 32]
    }
}
#[derive(Default)]
struct Store {
    writes: Cell<usize>,
    encrypted: RefCell<String>,
    mutation: RefCell<Option<ConfigMutation>>,
    deleted: Cell<bool>,
}
fn fixture() -> ConfigView {
    ConfigView::new(
        1,
        "Name".into(),
        "key".into(),
        Provider::Generic,
        "source".into(),
        true,
        "description".into(),
    )
}
impl WebhookConfigStore for Store {
    async fn list_owned(
        &self,
        user_id: i64,
        page: &ConfigPage,
    ) -> ConfigResult<(Vec<ConfigView>, u64)> {
        assert_eq!(page.offset, (page.page - 1) * page.size);
        Ok(if user_id == 7 && !self.deleted.get() {
            (vec![fixture()], 1)
        } else {
            (vec![], 0)
        })
    }
    async fn get_owned(&self, user_id: i64, id: i64) -> ConfigResult<Option<ConfigView>> {
        Ok((user_id == 7 && id == 1 && !self.deleted.get()).then(fixture))
    }
    async fn insert(&self, c: &NewConfig) -> ConfigResult<ConfigView> {
        assert_eq!(c.user_id, 7);
        assert_eq!(c.at, now());
        assert!(c.enabled);
        self.writes.set(self.writes.get() + 1);
        *self.encrypted.borrow_mut() = c.encrypted_secret.clone();
        Ok(ConfigView::new(
            1,
            c.name.clone(),
            c.key.clone(),
            c.provider,
            c.source.clone(),
            c.enabled,
            c.description.clone(),
        ))
    }
    async fn mutate_owned(
        &self,
        user_id: i64,
        id: i64,
        mutation: &ConfigMutation,
        at: DateTime<Utc>,
    ) -> ConfigResult<Option<ConfigView>> {
        assert_eq!(at, now());
        let Some(row) = self.get_owned(user_id, id).await? else {
            return Ok(None);
        };
        self.writes.set(self.writes.get() + 1);
        *self.mutation.borrow_mut() = Some(match mutation {
            ConfigMutation::Patch(p) => ConfigMutation::Patch(ConfigPatch {
                name: p.name.clone(),
                provider: p.provider,
                source: p.source.clone(),
                encrypted_secret: p.encrypted_secret.clone(),
                description: p.description.clone(),
            }),
            ConfigMutation::Enabled(enabled) => ConfigMutation::Enabled(*enabled),
            ConfigMutation::Delete => {
                self.deleted.set(true);
                ConfigMutation::Delete
            }
        });
        Ok(Some(row))
    }
}
fn service() -> WebhookConfigService<Store, Runtime> {
    WebhookConfigService {
        store: Store::default(),
        runtime: Runtime {
            random_calls: Cell::new(0),
            fail: false,
        },
    }
}
fn body() -> ConfigRequest {
    ConfigRequest {
        name: "Name".into(),
        provider: "generic".into(),
        secret: "credential".into(),
        ..Default::default()
    }
}
#[tokio::test]
async fn creation_defaults_key_encryption_and_public_projection_are_shared() {
    let s = service();
    let response = s.create(7, body()).await.unwrap();
    assert_eq!(response.key, hex::encode([1; 16]));
    assert!(response.enabled);
    assert!(
        serde_json::to_value(&response)
            .unwrap()
            .get("url")
            .is_none()
    );
    let encrypted = s.store.encrypted.borrow().clone();
    assert_ne!(encrypted, "credential");
    assert_eq!(
        crate::security::decrypt(&encrypted, &[9; 32]).unwrap(),
        "credential"
    );
    let json = serde_json::to_value(response).unwrap();
    assert!(json.get("secret").is_none());
    assert!(json.get("encryptedSecret").is_none());
    let mut empty = body();
    empty.secret.clear();
    s.create(7, empty).await.unwrap();
    assert!(s.store.encrypted.borrow().is_empty());
    assert_eq!(s.runtime.random_calls.get(), 3); // key+nonce, then key only
}
#[tokio::test]
async fn patches_are_sparse_empty_is_read_and_secret_never_crosses_store_in_plaintext() {
    let s = service();
    s.update(7, "1", ConfigRequest::default()).await.unwrap();
    assert_eq!(s.store.writes.get(), 0);
    s.update(
        7,
        "1",
        ConfigRequest {
            name: "New".into(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    {
        let command = s.store.mutation.borrow();
        let Some(ConfigMutation::Patch(p)) = command.as_ref() else {
            panic!()
        };
        assert_eq!(p.name.as_deref(), Some("New"));
        assert!(
            p.provider.is_none()
                && p.source.is_none()
                && p.encrypted_secret.is_none()
                && p.description.is_none()
        );
    }
    s.update(
        7,
        "1",
        ConfigRequest {
            secret: "replacement".into(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let command = s.store.mutation.borrow();
    let Some(ConfigMutation::Patch(p)) = command.as_ref() else {
        panic!()
    };
    assert_eq!(
        crate::security::decrypt(p.encrypted_secret.as_ref().unwrap(), &[9; 32]).unwrap(),
        "replacement"
    );
    assert!(p.name.is_none());
}
#[tokio::test]
async fn validation_scope_soft_deletion_and_runtime_failure_precede_writes() {
    let s = service();
    assert!(matches!(
        s.create(7, ConfigRequest::default()).await,
        Err(ConfigError::InvalidInput(_))
    ));
    assert!(matches!(
        s.update(
            7,
            "1",
            ConfigRequest {
                provider: "unknown".into(),
                ..Default::default()
            }
        )
        .await,
        Err(ConfigError::InvalidInput(_))
    ));
    for value in ["0", "-1", "bad", "9223372036854775808"] {
        assert!(matches!(
            s.get(7, value).await,
            Err(ConfigError::InvalidInput(_))
        ));
        assert!(s.enabled(7, value, false).await.is_err());
    }
    for uid in [8, 9] {
        assert!(matches!(s.get(uid, "1").await, Err(ConfigError::NotFound)));
        assert!(matches!(
            s.update(
                uid,
                "1",
                ConfigRequest {
                    name: "Other".into(),
                    ..Default::default()
                }
            )
            .await,
            Err(ConfigError::NotFound)
        ));
        assert!(matches!(
            s.delete(uid, "1").await,
            Err(ConfigError::NotFound)
        ));
    }
    assert_eq!(s.store.writes.get(), 0);
    s.enabled(7, "1", false).await.unwrap();
    assert!(matches!(
        *s.store.mutation.borrow(),
        Some(ConfigMutation::Enabled(false))
    ));
    s.delete(7, "1").await.unwrap();
    assert!(matches!(s.get(7, "1").await, Err(ConfigError::NotFound)));
    assert!(matches!(s.delete(7, "1").await, Err(ConfigError::NotFound)));
    assert_eq!(
        s.list(7, PaginationQuery::default()).await.unwrap().total,
        0
    );
    let failing = WebhookConfigService {
        store: Store::default(),
        runtime: Runtime {
            fail: true,
            random_calls: Cell::new(0),
        },
    };
    assert!(matches!(
        failing.create(7, body()).await,
        Err(ConfigError::Internal(_))
    ));
    assert_eq!(failing.store.writes.get(), 0);
}
