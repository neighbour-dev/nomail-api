use super::*;
use crate::webhook::{
    self, Authentication, DeduplicationKey, MessageLevel, PlatformEvent, Provider, ReceiveOutcome,
    WebhookConfig, WebhookRequest,
};
use chrono::{DateTime, Duration, Utc};
use serde_json::json;
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
};
use uuid::Uuid;

#[derive(Default)]
struct Store {
    rows: RefCell<HashMap<Uuid, Message>>,
    configs: RefCell<HashMap<i64, (i64, String, bool)>>,
    writes: Cell<usize>,
    last_ids: RefCell<Vec<Uuid>>,
    last_query: RefCell<Option<MessageQuery>>,
}
impl Store {
    fn new() -> Self {
        let store = Self::default();
        store
            .configs
            .borrow_mut()
            .insert(1, (7, "First".into(), false));
        store
            .configs
            .borrow_mut()
            .insert(2, (8, "Other".into(), false));
        store
    }
    fn visible(&self, user_id: i64, m: &Message) -> bool {
        m.deleted_at.is_none()
            && self
                .configs
                .borrow()
                .get(&m.webhook_config_id)
                .is_some_and(|(uid, _, deleted)| *uid == user_id && !*deleted)
    }
    fn joined(&self, m: &Message) -> MessageWithConfig {
        MessageWithConfig {
            message: m.clone(),
            config_name: self.configs.borrow()[&m.webhook_config_id].1.clone(),
        }
    }
}
fn patch(m: &mut Message, p: &StatePatch) {
    if let Some(read) = p.read {
        m.status = read.status().as_str().into();
        m.read_at = read.read_at();
    }
    if let Some(pinned) = p.pinned {
        m.pinned = pinned;
    }
    m.updated_at = p.updated_at;
}
impl MessageStore for Store {
    async fn list_owned(
        &self,
        user_id: i64,
        q: &MessageQuery,
    ) -> MessageResult<(Vec<MessageListItem>, u64)> {
        *self.last_query.borrow_mut() = Some(q.clone());
        let rows = self.rows.borrow();
        let mut selected: Vec<_> = rows.values().filter(|m| self.visible(user_id, m)).collect();
        selected.sort_by_key(|m| std::cmp::Reverse(m.received_at));
        let total = selected.len() as u64;
        Ok((
            selected
                .into_iter()
                .skip(usize::try_from(q.offset).unwrap_or(usize::MAX))
                .take(q.page_size as usize)
                .map(|m| MessageListItem {
                    id: m.id,
                    title: m.common.title.clone(),
                    summary: crate::pagination::summarize(&m.common.content),
                    source: m.common.source.clone().unwrap_or_default(),
                    level: m.common.level,
                    status: m.status.clone(),
                    pinned: m.pinned,
                    webhook_config_id: m.webhook_config_id,
                    received_at: m.received_at,
                    expires_at: m.expires_at,
                })
                .collect(),
            total,
        ))
    }
    async fn count_unread_owned(&self, user_id: i64) -> MessageResult<u64> {
        Ok(self
            .rows
            .borrow()
            .values()
            .filter(|m| self.visible(user_id, m) && m.status == "unread")
            .count() as u64)
    }
    async fn get_owned(&self, user_id: i64, id: Uuid) -> MessageResult<Option<MessageWithConfig>> {
        Ok(self
            .rows
            .borrow()
            .get(&id)
            .filter(|m| self.visible(user_id, m))
            .map(|m| self.joined(m)))
    }
    async fn get_raw_owned(&self, user_id: i64, id: Uuid) -> MessageResult<Option<RawContent>> {
        Ok(self
            .rows
            .borrow()
            .get(&id)
            .filter(|m| self.visible(user_id, m))
            .map(|m| RawContent {
                text: m.raw_payload.clone(),
                content_type: m.content_type.clone(),
            }))
    }
    async fn update_owned(
        &self,
        user_id: i64,
        id: Uuid,
        p: &StatePatch,
    ) -> MessageResult<Option<MessageWithConfig>> {
        self.writes.set(self.writes.get() + 1);
        let mut rows = self.rows.borrow_mut();
        let Some(m) = rows.get_mut(&id).filter(|m| self.visible(user_id, m)) else {
            return Ok(None);
        };
        patch(m, p);
        Ok(Some(self.joined(m)))
    }
    async fn mutate_many_owned(
        &self,
        user_id: i64,
        ids: &[Uuid],
        command: &MessageMutation,
    ) -> MessageResult<u64> {
        self.writes.set(self.writes.get() + 1);
        *self.last_ids.borrow_mut() = ids.to_vec();
        let mut rows = self.rows.borrow_mut();
        let mut affected = 0;
        let mut seen = HashSet::new();
        for id in ids {
            if !seen.insert(*id) {
                continue;
            }
            if let Some(m) = rows.get_mut(id).filter(|m| self.visible(user_id, m)) {
                match command {
                    MessageMutation::State(p) => patch(m, p),
                    MessageMutation::SoftDelete { at } => {
                        m.deleted_at = Some(*at);
                        m.updated_at = *at;
                    }
                }
                affected += 1;
            }
        }
        Ok(affected)
    }
    async fn insert_once(&self, m: &Message) -> MessageResult<InsertOutcome> {
        if let Some(key) = &m.metadata.deduplication_key
            && let Some(old) = self.rows.borrow().values().find(|old| {
                old.webhook_config_id == m.webhook_config_id
                    && old.metadata.deduplication_key.as_ref() == Some(key)
            })
        {
            return Ok(InsertOutcome::Duplicate(old.id));
        }
        self.writes.set(self.writes.get() + 1);
        if self.rows.borrow().contains_key(&m.id) {
            return Err(MessageError::Conflict("duplicate id"));
        }
        self.rows.borrow_mut().insert(m.id, m.clone());
        Ok(InsertOutcome::Created(m.id))
    }
}
fn now() -> DateTime<Utc> {
    DateTime::from_timestamp(1000, 0).unwrap()
}
fn event(id: Option<&str>, level: &str) -> PlatformEvent {
    let body=json!({"id":id,"title":" Title ","content":"hello\n 世界","level":level,"unknown":{"keep":true}}).to_string();
    let config = WebhookConfig::new(Provider::Generic, Authentication::None, 1000);
    let headers = vec![];
    let ReceiveOutcome::Event(event) = webhook::receive(
        &config,
        &WebhookRequest {
            headers: &headers,
            body: body.as_bytes(),
        },
    )
    .unwrap() else {
        panic!()
    };
    *event
}

#[tokio::test]
async fn complete_message_and_initial_state_have_one_source_and_only_creation_emits_an_event() {
    let service = MessageService {
        store: Store::new(),
    };
    let raw = event(Some("key"), "high");
    let original = raw.clone();
    let id = Uuid::from_u128(1);
    let result = service
        .accept(1, raw, Some("source"), id, now())
        .await
        .unwrap();
    assert!(!result.outcome.duplicate());
    let created = result.created_event.unwrap();
    assert_eq!(created.id, id);
    assert_eq!(created.level, MessageLevel::High);
    assert!(!created.pinned);
    assert_eq!(created.status, "unread");
    let record = service.store.rows.borrow()[&id].clone();
    assert_eq!(record.common.title, "Title");
    assert_eq!(record.metadata, original.metadata);
    assert_eq!(record.platform_data, original.data);
    assert_eq!(record.raw_payload, original.raw.to_readable_string());
    assert_eq!(record.received_at, now());
    assert_eq!(record.updated_at, now());
    assert_eq!(
        record.expires_at,
        Some(now() + Duration::days(DEFAULT_RETENTION_DAYS))
    );
    assert_eq!(created.expires_at, record.expires_at);
    service
        .delete(7, id, now() + Duration::seconds(1))
        .await
        .unwrap();
    let duplicate = service
        .accept(1, original, None, Uuid::from_u128(2), now())
        .await
        .unwrap();
    assert_eq!(duplicate.outcome.id(), id);
    assert!(duplicate.created_event.is_none());
    assert!(matches!(
        service.get(7, id).await,
        Err(MessageError::NotFound(_))
    ));
}

#[tokio::test]
async fn state_updates_are_sparse_preserve_content_and_use_one_operation_timestamp() {
    let service = MessageService {
        store: Store::new(),
    };
    let id = Uuid::from_u128(1);
    service
        .accept(1, event(None, "urgent"), None, id, now())
        .await
        .unwrap();
    let before = service.store.rows.borrow()[&id].clone();
    let read_time = now() + Duration::seconds(1);
    service
        .update(
            7,
            id,
            UpdateRequest {
                status: Some("read".into()),
                pinned: None,
            },
            read_time,
        )
        .await
        .unwrap();
    let pinned = service
        .update(
            7,
            id,
            UpdateRequest {
                status: None,
                pinned: Some(true),
            },
            read_time + Duration::seconds(1),
        )
        .await
        .unwrap();
    assert!(pinned.pinned);
    assert_eq!(pinned.status, "read");
    assert_eq!(pinned.read_at, Some(read_time));
    assert_eq!(pinned.platform_data, before.platform_data);
    assert_eq!(pinned.metadata.event_type, before.metadata.event_type);
    assert_eq!(pinned.created_at, before.received_at);
    let repeated = service
        .update(
            7,
            id,
            UpdateRequest {
                status: Some("read".into()),
                pinned: None,
            },
            read_time + Duration::seconds(2),
        )
        .await
        .unwrap();
    assert_eq!(repeated.read_at, Some(read_time + Duration::seconds(2)));
    let unread = service
        .update(
            7,
            id,
            UpdateRequest {
                status: Some("unread".into()),
                pinned: None,
            },
            read_time + Duration::seconds(3),
        )
        .await
        .unwrap();
    assert!(unread.pinned);
    assert!(unread.read_at.is_none());
    assert_eq!(
        service.store.rows.borrow()[&id].raw_payload,
        before.raw_payload
    );
    let writes = service.store.writes.get();
    assert!(
        service
            .update(
                7,
                id,
                UpdateRequest {
                    status: None,
                    pinned: None
                },
                now()
            )
            .await
            .is_err()
    );
    assert_eq!(service.store.writes.get(), writes);
}

#[tokio::test]
async fn batch_validation_precedes_writes_and_scope_and_duplicate_semantics_are_preserved() {
    let service = MessageService {
        store: Store::new(),
    };
    let a = Uuid::from_u128(1);
    let b = Uuid::from_u128(2);
    let foreign = Uuid::from_u128(3);
    for (config, id) in [(1, a), (1, b), (2, foreign)] {
        service
            .accept(config, event(None, "normal"), None, id, now())
            .await
            .unwrap();
    }
    let writes = service.store.writes.get();
    for request in [
        BatchRequest {
            ids: vec![],
            action: "pin".into(),
        },
        BatchRequest {
            ids: vec![a.to_string(); 101],
            action: "pin".into(),
        },
        BatchRequest {
            ids: vec![a.to_string(), "invalid".into()],
            action: "pin".into(),
        },
        BatchRequest {
            ids: vec![a.to_string()],
            action: "invalid".into(),
        },
    ] {
        assert!(matches!(
            service.batch(7, request, now()).await,
            Err(MessageError::InvalidInput(_))
        ));
        assert_eq!(service.store.writes.get(), writes);
    }
    let at = now() + Duration::seconds(5);
    let result = service
        .batch(
            7,
            BatchRequest {
                ids: vec![
                    a.to_string(),
                    a.to_string(),
                    b.to_string(),
                    foreign.to_string(),
                    Uuid::from_u128(99).to_string(),
                ],
                action: "mark_read".into(),
            },
            at,
        )
        .await
        .unwrap();
    assert_eq!(result.affected, 2);
    assert_eq!(service.store.last_ids.borrow().len(), 4);
    for id in [a, b] {
        let row = service.get(7, id).await.unwrap();
        assert_eq!(row.read_at, Some(at));
        assert_eq!(row.updated_at, at);
    }
    assert_eq!(service.store.rows.borrow()[&foreign].status, "unread");
    assert!(matches!(
        service
            .update(
                7,
                foreign,
                UpdateRequest {
                    status: None,
                    pinned: Some(true)
                },
                at
            )
            .await,
        Err(MessageError::NotFound(_))
    ));
    assert!(matches!(
        service.delete(7, foreign, at).await,
        Err(MessageError::NotFound(_))
    ));
}

#[tokio::test]
async fn expiry_is_not_an_implicit_visibility_filter_and_raw_is_a_separate_view() {
    let service = MessageService {
        store: Store::new(),
    };
    for (index, level) in ["low", "normal", "high", "urgent"].into_iter().enumerate() {
        let id = Uuid::from_u128(index as u128 + 1);
        service
            .accept(1, event(None, level), None, id, now())
            .await
            .unwrap();
        assert_eq!(
            service.store.rows.borrow()[&id].expires_at,
            Some(now() + Duration::days(3))
        );
    }
    assert_eq!(
        service.list(7, ListQuery::default()).await.unwrap().total,
        4
    );
    let id = Uuid::from_u128(1);
    service
        .store
        .rows
        .borrow_mut()
        .get_mut(&id)
        .unwrap()
        .expires_at = Some(now() - Duration::days(1));
    assert!(service.get(7, id).await.is_ok());
    service
        .store
        .rows
        .borrow_mut()
        .get_mut(&id)
        .unwrap()
        .content_type
        .clear();
    assert_eq!(
        service.raw(7, id).await.unwrap().content_type,
        "application/octet-stream"
    );
    service
        .store
        .rows
        .borrow_mut()
        .get_mut(&id)
        .unwrap()
        .raw_payload
        .clear();
    assert!(matches!(
        service.raw(7, id).await,
        Err(MessageError::NotFound(_))
    ));
    service.store.configs.borrow_mut().get_mut(&1).unwrap().2 = true;
    assert_eq!(
        service.list(7, ListQuery::default()).await.unwrap().total,
        0
    );
}

#[tokio::test]
async fn deduplication_keeps_full_keys_scopes_and_null_key_independence() {
    let service = MessageService {
        store: Store::new(),
    };
    let key = "x".repeat(400);
    for (index, scope) in ["first", "second"].into_iter().enumerate() {
        let mut value = event(None, "normal");
        value.metadata.deduplication_key = Some(DeduplicationKey {
            scope: scope.into(),
            value: key.clone(),
        });
        let accepted = service
            .accept(1, value, None, Uuid::from_u128(index as u128 + 1), now())
            .await
            .unwrap();
        assert!(!accepted.outcome.duplicate());
    }
    for id in [3, 4] {
        assert!(
            !service
                .accept(1, event(None, "normal"), None, Uuid::from_u128(id), now())
                .await
                .unwrap()
                .outcome
                .duplicate()
        );
    }
    assert_eq!(service.store.rows.borrow().len(), 4);
}

#[test]
fn typed_queries_share_pagination_boolean_level_time_validation_and_search_semantics() {
    let query:ListQuery=serde_json::from_value(json!({"page":"0","pageSize":"999","query":" %hello_ ","status":" read ","level":"HIGH","pinned":"T","webhookConfigId":"2","from":"2026-01-01T01:00:00+01:00","to":"2026-01-02T00:00:00Z"})).unwrap();
    let parsed = query.parsed().unwrap();
    assert_eq!(parsed.page, 1);
    assert_eq!(parsed.page_size, 100);
    assert_eq!(parsed.text.as_deref(), Some("%hello_"));
    assert_eq!(parsed.status, Some(ReadStatus::Read));
    assert_eq!(parsed.level, Some(MessageLevel::High));
    assert_eq!(parsed.pinned, Some(true));
    assert_eq!(parsed.webhook_config_id, Some(2));
    assert_eq!(
        parsed.from.unwrap().to_rfc3339(),
        "2026-01-01T00:00:00+00:00"
    );
    for value in [
        json!({"level":"critical"}),
        json!({"status":"invalid"}),
        json!({"pinned":"invalid"}),
        json!({"webhookConfigId":"0"}),
        json!({"from":"bad"}),
        json!({"from":"2026-01-02T00:00:00Z","to":"2026-01-01T00:00:00Z"}),
        json!({"page":i64::MAX.to_string(),"pageSize":"100"}),
    ] {
        assert!(
            serde_json::from_value::<ListQuery>(value)
                .unwrap()
                .parsed()
                .is_err()
        );
    }
    assert_eq!(
        crate::pagination::summarize("  hello\n世界  "),
        "hello 世界"
    );
    assert_eq!(
        crate::pagination::summarize(&"中".repeat(200))
            .chars()
            .count(),
        160
    );
}
