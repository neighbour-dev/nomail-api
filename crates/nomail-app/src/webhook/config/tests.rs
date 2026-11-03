use super::*;
use crate::webhook::Provider;
use chrono::{DateTime, Utc};
use std::cell::{Cell, RefCell};

fn now() -> DateTime<Utc> {
    DateTime::from_timestamp(1000, 0).unwrap()
}
fn id() -> ConfigId {
    ConfigId::new(1).unwrap()
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
fn fixture() -> ConfigRecord {
    ConfigRecord::new(
        1,
        "Name".into(),
        "key".into(),
        Provider::Generic,
        crate::security::encrypt(
            r#"{"provider":"generic","credentials":{"type":"none"}}"#,
            &[9; 32],
            &[0; 12],
        )
        .unwrap(),
        true,
        "description".into(),
    )
}
impl WebhookConfigStore for Store {
    async fn list_owned(
        &self,
        user_id: i64,
        page: &ConfigPage,
    ) -> ConfigResult<(Vec<ConfigRecord>, u64)> {
        assert_eq!(page.offset(), (page.page() - 1) * page.size());
        Ok(if user_id == 7 && !self.deleted.get() {
            (vec![fixture()], 1)
        } else {
            (vec![], 0)
        })
    }
    async fn get_owned(&self, user_id: i64, id: i64) -> ConfigResult<Option<ConfigRecord>> {
        Ok((user_id == 7 && id == 1 && !self.deleted.get()).then(fixture))
    }
    async fn insert(&self, c: &NewConfig) -> ConfigResult<ConfigRecord> {
        assert_eq!(c.user_id, 7);
        assert_eq!(c.at, now());
        assert!(c.enabled);
        self.writes.set(self.writes.get() + 1);
        *self.encrypted.borrow_mut() = c.encrypted_platform.clone();
        Ok(ConfigRecord::new(
            1,
            c.name.clone(),
            c.key.clone(),
            c.provider,
            c.encrypted_platform.clone(),
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
    ) -> ConfigResult<Option<ConfigRecord>> {
        assert_eq!(at, now());
        let Some(row) = self.get_owned(user_id, id).await? else {
            return Ok(None);
        };
        self.writes.set(self.writes.get() + 1);
        *self.mutation.borrow_mut() = Some(match mutation {
            ConfigMutation::Patch(p) => ConfigMutation::Patch(ConfigPatch {
                name: p.name.clone(),
                platform: p.platform.as_ref().map(|p| EncryptedPlatform {
                    provider: p.provider,
                    ciphertext: p.ciphertext.clone(),
                }),
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
fn body() -> CreateConfig {
    CreateConfig::new("Name".into(), platform("credential"), String::new()).unwrap()
}
fn platform(secret: &str) -> crate::webhook::PlatformConfig {
    serde_json::from_value(serde_json::json!({"provider":"generic","credentials":{"type":"hmacSha256","secret":secret}})).unwrap()
}
#[tokio::test]
async fn creation_defaults_key_encryption_and_public_projection_are_shared() {
    let s = service();
    let response = s.create(7, body()).await.unwrap();
    assert_eq!(response.key, hex::encode([1; 16]));
    assert!(response.enabled);
    let encrypted = s.store.encrypted.borrow().clone();
    assert_ne!(encrypted, "credential");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            &crate::security::decrypt(&encrypted, &[9; 32]).unwrap()
        )
        .unwrap()["credentials"]["secret"],
        "credential"
    );
    let mut empty = body();
    empty.platform =
        crate::webhook::PlatformConfig::Generic(crate::webhook::GenericCredentials::None {});
    s.create(7, empty).await.unwrap();
    assert!(!s.store.encrypted.borrow().is_empty());
    assert_eq!(s.runtime.random_calls.get(), 4); // key+nonce for each complete configuration
}
#[tokio::test]
async fn patches_are_sparse_empty_is_read_and_secret_never_crosses_store_in_plaintext() {
    let s = service();
    s.update(7, id(), UpdateConfig::default()).await.unwrap();
    assert_eq!(s.store.writes.get(), 0);
    s.update(
        7,
        id(),
        UpdateConfig::new(Some("New".into()), None, None).unwrap(),
    )
    .await
    .unwrap();
    {
        let command = s.store.mutation.borrow();
        let Some(ConfigMutation::Patch(p)) = command.as_ref() else {
            panic!()
        };
        assert_eq!(p.name.as_deref(), Some("New"));
        assert!(p.platform.is_none() && p.description.is_none());
    }
    s.update(
        7,
        id(),
        UpdateConfig::new(None, Some(platform("replacement")), None).unwrap(),
    )
    .await
    .unwrap();
    let command = s.store.mutation.borrow();
    let Some(ConfigMutation::Patch(p)) = command.as_ref() else {
        panic!()
    };
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            &crate::security::decrypt(&p.platform.as_ref().unwrap().ciphertext, &[9; 32]).unwrap()
        )
        .unwrap()["credentials"]["secret"],
        "replacement"
    );
    assert!(p.name.is_none());
}
#[tokio::test]
async fn validation_scope_soft_deletion_and_runtime_failure_precede_writes() {
    let s = service();
    assert!(matches!(
        CreateConfig::new(String::new(), platform("secret"), String::new()),
        Err(ConfigError::InvalidInput(_))
    ));
    assert!(matches!(
        UpdateConfig::new(None, Some(platform("")), None),
        Err(ConfigError::InvalidInput(_))
    ));
    for value in [0, -1, i64::MIN] {
        assert!(matches!(
            ConfigId::new(value),
            Err(ConfigError::InvalidInput(_))
        ));
    }
    assert_eq!(ConfigId::new(i64::MAX).unwrap().get(), i64::MAX);
    for uid in [8, 9] {
        assert!(matches!(s.get(uid, id()).await, Err(ConfigError::NotFound)));
        assert!(matches!(
            s.update(
                uid,
                id(),
                UpdateConfig::new(Some("Other".into()), None, None).unwrap()
            )
            .await,
            Err(ConfigError::NotFound)
        ));
        assert!(matches!(
            s.delete(uid, id()).await,
            Err(ConfigError::NotFound)
        ));
    }
    assert_eq!(s.store.writes.get(), 0);
    s.enabled(7, id(), false).await.unwrap();
    assert!(matches!(
        *s.store.mutation.borrow(),
        Some(ConfigMutation::Enabled(false))
    ));
    s.delete(7, id()).await.unwrap();
    assert!(matches!(s.get(7, id()).await, Err(ConfigError::NotFound)));
    assert!(matches!(
        s.delete(7, id()).await,
        Err(ConfigError::NotFound)
    ));
    assert_eq!(
        s.list(7, ConfigPage::new(1, 20).unwrap())
            .await
            .unwrap()
            .total,
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
