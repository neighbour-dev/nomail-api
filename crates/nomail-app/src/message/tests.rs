use super::*;
use crate::webhook::{
    self, Authentication, DeduplicationKey, EventMetadata, MessageLevel, PlatformEvent, Provider,
    RawWebhook, ReceiveOutcome, WebhookConfig, WebhookRequest, parse_raw,
};
use chrono::{DateTime, Duration, Utc};
use serde_json::{Value, json};
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
    fn seed(&self, config_id: i64, event: PlatformEvent, id: Uuid) {
        let message = Message::from_received_event(config_id, event, id, now()).unwrap();
        self.rows.borrow_mut().insert(id, message);
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
    async fn cursor_owned(
        &self,
        user_id: i64,
        q: &MessageCursorQuery,
    ) -> MessageResult<Vec<MessagePreview>> {
        let rows = self.rows.borrow();
        let mut selected: Vec<_> = rows
            .values()
            .filter(|m| {
                if !self.visible(user_id, m) {
                    return false;
                }
                let position = MessagePosition {
                    received_at: m.received_at,
                    id: m.id,
                };
                match q.window() {
                    MessageWindow::Latest => true,
                    MessageWindow::Before(boundary) => position < boundary,
                    MessageWindow::After(boundary) => position > boundary,
                }
            })
            .collect();
        selected.sort_by_key(|m| (m.received_at, m.id));
        if !q.window().ascending() {
            selected.reverse();
        }
        Ok(selected
            .into_iter()
            .take(q.fetch_limit() as usize)
            .map(|m| MessagePreview {
                id: m.id,
                title: m.common.title.clone(),
                summary: m.common.summary.clone(),
                level: m.common.level,
                status: m.status.clone(),
                pinned: m.pinned,
                webhook_config_id: m.webhook_config_id,
                received_at: m.received_at,
                expires_at: m.expires_at,
            })
            .collect())
    }
    async fn list_owned(
        &self,
        user_id: i64,
        q: &MessageQuery,
    ) -> MessageResult<(Vec<MessagePreview>, u64)> {
        *self.last_query.borrow_mut() = Some(q.clone());
        let rows = self.rows.borrow();
        let mut selected: Vec<_> = rows.values().filter(|m| self.visible(user_id, m)).collect();
        selected.sort_by_key(|m| std::cmp::Reverse(m.received_at));
        let total = selected.len() as u64;
        Ok((
            selected
                .into_iter()
                .skip(usize::try_from(q.offset()).unwrap_or(usize::MAX))
                .take(q.page_size() as usize)
                .map(|m| MessagePreview {
                    id: m.id,
                    title: m.common.title.clone(),
                    summary: m.common.summary.clone(),
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
                header: m
                    .raw_headers
                    .iter()
                    .map(|header| serde_json::to_value(header).unwrap())
                    .collect(),
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
}
fn now() -> DateTime<Utc> {
    DateTime::from_timestamp(1000, 0).unwrap()
}

#[tokio::test]
async fn cursor_service_builds_continuations_in_storage_order_without_counting() {
    let service = MessageService {
        store: Store::new(),
    };
    for i in 1..=5 {
        service
            .store
            .seed(1, event(None, "normal"), Uuid::from_u128(i));
    }
    let query = |window| MessageCursorQuery::new(MessageFilters::default(), window, 2).unwrap();
    let first = service
        .cursor(7, query(MessageWindow::Latest))
        .await
        .unwrap();
    assert_eq!(
        first.items.iter().map(|m| m.id).collect::<Vec<_>>(),
        vec![Uuid::from_u128(5), Uuid::from_u128(4)]
    );
    assert!(first.has_more);
    assert_eq!(first.next, first.end);
    assert_eq!(first.start.unwrap().id, Uuid::from_u128(5));
    service.delete(7, Uuid::from_u128(4), now()).await.unwrap();
    let older = service
        .cursor(7, query(MessageWindow::Before(first.end.unwrap())))
        .await
        .unwrap();
    assert_eq!(
        older.items.iter().map(|m| m.id).collect::<Vec<_>>(),
        vec![Uuid::from_u128(3), Uuid::from_u128(2)]
    );
    let added = service
        .cursor(
            7,
            query(MessageWindow::After(MessagePosition {
                received_at: now(),
                id: Uuid::from_u128(2),
            })),
        )
        .await
        .unwrap();
    assert_eq!(
        added.items.iter().map(|m| m.id).collect::<Vec<_>>(),
        vec![Uuid::from_u128(3), Uuid::from_u128(5)]
    );
    assert!(!added.has_more);
    assert!(added.next.is_none());
    let empty = service
        .cursor(8, query(MessageWindow::Latest))
        .await
        .unwrap();
    assert!(empty.items.is_empty());
    assert!(!empty.has_more);
    assert!(empty.next.is_none() && empty.start.is_none() && empty.end.is_none());
    for limit in [0, 101, u64::MAX] {
        assert!(
            MessageCursorQuery::new(MessageFilters::default(), MessageWindow::Latest, limit)
                .is_err()
        );
    }
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

#[test]
fn received_message_factory_preserves_platform_content_and_sets_inbox_defaults() {
    let raw = event(Some("key"), "high");
    let original = raw.clone();
    let id = Uuid::from_u128(1);
    let record = Message::from_received_event(1, raw, id, now()).unwrap();
    let created = MessageAccepted::from(&record);
    assert_eq!(created.id, id);
    assert_eq!(created.level, MessageLevel::High);
    assert!(!created.pinned);
    assert_eq!(created.status, "unread");
    assert_eq!(record.common.title, "Title");
    assert!(!record.common.summary.is_empty());
    assert_eq!(record.webhook_config_id, 1);
    assert_eq!(record.provider, original.provider());
    assert_eq!(record.metadata, original.metadata);
    assert_eq!(record.platform_data().ok(), original.data);
    assert_eq!(record.raw_headers, original.raw.header);
    assert_eq!(record.raw_payload, original.raw.stored_payload());
    assert_eq!(record.received_at, now());
    assert_eq!(record.updated_at, now());
    assert_eq!(record.content_type, original.raw.payload_content_type());
    assert!(record.deleted_at.is_none());
    assert!(record.read_at.is_none());
    assert_eq!(
        record.expires_at,
        Some(now() + Duration::days(DEFAULT_RETENTION_DAYS))
    );
    assert_eq!(created.expires_at, record.expires_at);
}

#[test]
fn received_message_factory_rejects_an_unrepresentable_expiry() {
    assert!(matches!(
        Message::from_received_event(1, event(None, "normal"), Uuid::from_u128(1), DateTime::<Utc>::MAX_UTC),
        Err(MessageError::Internal(message)) if message == "invalid message expiry"
    ));
}

#[tokio::test]
async fn summaries_are_bounded_receipt_snapshots_and_native_fields_remain_intact() {
    for (text, expected) in [
        (json!("  hello\n\t世界  "), "hello 世界".to_owned()),
        (json!("中".repeat(30)), "中".repeat(30)),
        (json!("中".repeat(31)), format!("{}…", "中".repeat(29))),
        (json!("😀".repeat(31)), format!("{}…", "😀".repeat(29))),
        (json!(" \n\t "), "无法解析消息内容".into()),
        (json!(42), "无法解析消息内容".into()),
        (Value::Null, "无法解析消息内容".into()),
    ] {
        let payload = json!({"title":"Summary","source":"native source","content":text});
        let raw =
            RawWebhook::from_stored(vec![], &payload.to_string(), "application/json").unwrap();
        let data = parse_raw(Provider::Generic, &raw).ok();
        let event = PlatformEvent {
            provider: Provider::Generic,
            metadata: EventMetadata {
                event_type: String::new(),
                version: None,
                deduplication_key: None,
            },
            data,
            raw,
        };
        let mut message =
            Message::from_received_event(1, event, Uuid::from_u128(1), now()).unwrap();
        assert_eq!(message.common.summary, expected);
        assert!(message.common.summary.chars().count() <= 30);
        assert_eq!(
            serde_json::from_str::<Value>(&message.raw_payload).unwrap(),
            payload
        );
        let wire = serde_json::to_value(&message).unwrap();
        assert!(wire.get("source").is_none() && wire.get("content").is_none());
        // A later platform projection failure never recomputes the receipt snapshot.
        message.raw_payload = "{invalid".into();
        let view = MessageView::from_stored(MessageWithConfig {
            message,
            config_name: "Config".into(),
        });
        assert!(view.platform_data.is_none());
        assert_eq!(view.message.common.summary, expected);
    }
}

#[tokio::test]
async fn failed_platform_projection_keeps_details_raw_and_state_updates_available() {
    let service = MessageService {
        store: Store::new(),
    };
    let id = Uuid::from_u128(1);
    service.store.seed(1, event(None, "normal"), id);
    {
        let mut rows = service.store.rows.borrow_mut();
        let message = rows.get_mut(&id).unwrap();
        assert!(
            serde_json::to_value(&*message)
                .unwrap()
                .get("platform_data")
                .is_none()
        );
        message.raw_payload = json!({"title":"raw-only title","content":null}).to_string();
    }
    let detail = service.get(7, id).await.unwrap();
    let view = serde_json::to_value(detail.platform_data).unwrap();
    assert_eq!(
        view["event"]["payload"],
        json!({"title":"raw-only title","content":null})
    );
    assert_ne!(
        detail.message.common.title, "raw-only title",
        "stored common display content must not be regenerated"
    );
    service
        .store
        .rows
        .borrow_mut()
        .get_mut(&id)
        .unwrap()
        .provider = Provider::Telegram;
    let writes = service.store.writes.get();
    let detail = service.get(7, id).await.unwrap();
    assert!(detail.platform_data.is_none());
    assert!(detail.platform_data_error.is_some());
    let updated = service
        .update(7, id, UpdateMessage::new(None, Some(true)).unwrap(), now())
        .await
        .unwrap();
    assert!(updated.platform_data.is_none());
    assert!(updated.platform_data_error.is_some());
    assert_eq!(service.store.writes.get(), writes + 1);
    assert!(service.store.rows.borrow()[&id].pinned);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&service.raw(7, id).await.unwrap().text).unwrap()
            ["title"],
        "raw-only title"
    );
}

#[tokio::test]
async fn raw_queries_do_not_decode_payloads_or_normalize_content_types() {
    let service = MessageService {
        store: Store::new(),
    };
    let id = Uuid::from_u128(1);
    service.store.seed(1, event(None, "normal"), id);
    for (payload, content_type) in [
        (" {\n \"title\":\"中文\"\n}\n", "application/json"),
        ("{invalid JSON", "application/json"),
        ("", "text/plain"),
        (
            "text=%20one%20&text=two",
            "application/x-www-form-urlencoded",
        ),
        ("opaque", "application/custom; version=1"),
        ("", ""),
    ] {
        {
            let mut rows = service.store.rows.borrow_mut();
            let message = rows.get_mut(&id).unwrap();
            message.raw_payload = payload.into();
            message.content_type = content_type.into();
        }
        let raw = service.raw(7, id).await.unwrap();
        assert_eq!(raw.text, payload);
        assert_eq!(raw.content_type, content_type);
        assert_eq!(
            serde_json::to_value(raw.header).unwrap(),
            serde_json::to_value(&service.store.rows.borrow()[&id].raw_headers).unwrap()
        );
    }
}

#[tokio::test]
async fn deleting_an_owned_message_hides_it_without_rewriting_its_content() {
    let service = MessageService {
        store: Store::new(),
    };
    let id = Uuid::from_u128(1);
    service.store.seed(1, event(Some("key"), "normal"), id);
    let before = service.store.rows.borrow()[&id].clone();
    let at = now() + Duration::seconds(1);
    service.delete(7, id, at).await.unwrap();
    assert!(matches!(
        service.get(7, id).await,
        Err(MessageError::NotFound(_))
    ));
    let rows = service.store.rows.borrow();
    assert_eq!(rows[&id].deleted_at, Some(at));
    assert_eq!(rows[&id].updated_at, at);
    assert_eq!(rows[&id].common, before.common);
    assert_eq!(rows[&id].metadata, before.metadata);
    assert_eq!(rows[&id].raw_payload, before.raw_payload);
}

#[tokio::test]
async fn state_updates_are_sparse_preserve_content_and_use_one_operation_timestamp() {
    let service = MessageService {
        store: Store::new(),
    };
    let id = Uuid::from_u128(1);
    service.store.seed(1, event(None, "urgent"), id);
    let before = service.store.rows.borrow()[&id].clone();
    let read_time = now() + Duration::seconds(1);
    service
        .update(
            7,
            id,
            UpdateMessage::new(Some(ReadStatus::Read), None).unwrap(),
            read_time,
        )
        .await
        .unwrap();
    let pinned = service
        .update(
            7,
            id,
            UpdateMessage::new(None, Some(true)).unwrap(),
            read_time + Duration::seconds(1),
        )
        .await
        .unwrap()
        .message;
    assert!(pinned.pinned);
    assert_eq!(pinned.status, "read");
    assert_eq!(pinned.read_at, Some(read_time));
    assert_eq!(
        pinned.platform_data().unwrap(),
        before.platform_data().unwrap()
    );
    assert_eq!(pinned.metadata.event_type, before.metadata.event_type);
    assert_eq!(pinned.received_at, before.received_at);
    let repeated = service
        .update(
            7,
            id,
            UpdateMessage::new(Some(ReadStatus::Read), None).unwrap(),
            read_time + Duration::seconds(2),
        )
        .await
        .unwrap();
    assert_eq!(
        repeated.message.read_at,
        Some(read_time + Duration::seconds(2))
    );
    let unread = service
        .update(
            7,
            id,
            UpdateMessage::new(Some(ReadStatus::Unread), None).unwrap(),
            read_time + Duration::seconds(3),
        )
        .await
        .unwrap();
    assert!(unread.message.pinned);
    assert!(unread.message.read_at.is_none());
    assert_eq!(
        service.store.rows.borrow()[&id].raw_payload,
        before.raw_payload
    );
    let writes = service.store.writes.get();
    assert!(UpdateMessage::new(None, None).is_err());
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
        service.store.seed(config, event(None, "normal"), id);
    }
    let writes = service.store.writes.get();
    for ids in [vec![], vec![a; 101]] {
        assert!(matches!(
            BatchMessages::new(ids, BatchAction::Pin),
            Err(MessageError::InvalidInput(_))
        ));
        assert_eq!(service.store.writes.get(), writes);
    }
    let at = now() + Duration::seconds(5);
    let result = service
        .batch(
            7,
            BatchMessages::new(
                vec![a, a, b, foreign, Uuid::from_u128(99)],
                BatchAction::MarkRead,
            )
            .unwrap(),
            at,
        )
        .await
        .unwrap();
    assert_eq!(result, 2);
    assert_eq!(service.store.last_ids.borrow().len(), 4);
    for id in [a, b] {
        let row = service.get(7, id).await.unwrap().message;
        assert_eq!(row.read_at, Some(at));
        assert_eq!(row.updated_at, at);
    }
    assert_eq!(service.store.rows.borrow()[&foreign].status, "unread");
    assert!(matches!(
        service
            .update(
                7,
                foreign,
                UpdateMessage::new(None, Some(true)).unwrap(),
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
        service.store.seed(1, event(None, level), id);
        assert_eq!(
            service.store.rows.borrow()[&id].expires_at,
            Some(now() + Duration::days(3))
        );
    }
    assert_eq!(
        service
            .list(7, MessageQuery::default())
            .await
            .unwrap()
            .total,
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
    assert_eq!(service.raw(7, id).await.unwrap().content_type, "");
    service
        .store
        .rows
        .borrow_mut()
        .get_mut(&id)
        .unwrap()
        .raw_payload
        .clear();
    service
        .store
        .rows
        .borrow_mut()
        .get_mut(&id)
        .unwrap()
        .content_type = "text/plain".into();
    assert_eq!(service.raw(7, id).await.unwrap().text, "");
    service.store.configs.borrow_mut().get_mut(&1).unwrap().2 = true;
    assert_eq!(
        service
            .list(7, MessageQuery::default())
            .await
            .unwrap()
            .total,
        0
    );
}

#[test]
fn received_message_factory_preserves_full_deduplication_metadata() {
    let key = "x".repeat(400);
    for (index, scope) in ["first", "second"].into_iter().enumerate() {
        let mut value = event(None, "normal");
        value.metadata.deduplication_key = Some(DeduplicationKey {
            scope: scope.into(),
            value: key.clone(),
        });
        let message =
            Message::from_received_event(1, value, Uuid::from_u128(index as u128 + 1), now())
                .unwrap();
        let stored_key = message.metadata.deduplication_key.unwrap();
        assert_eq!(stored_key.scope, scope);
        assert_eq!(stored_key.value, key);
    }
    for id in [3, 4] {
        let message =
            Message::from_received_event(1, event(None, "normal"), Uuid::from_u128(id), now())
                .unwrap();
        assert!(message.metadata.deduplication_key.is_none());
        assert_eq!(message.id, Uuid::from_u128(id));
    }
}

#[test]
fn typed_query_construction_validates_invariants_without_wire_strings() {
    let filters = MessageFilters {
        text: Some(" %hello_ ".into()),
        status: Some(ReadStatus::Read),
        level: Some(MessageLevel::High),
        pinned: Some(true),
        webhook_config_id: Some(2),
        ..Default::default()
    };
    let query = MessageQuery::new(filters, 2, 20).unwrap();
    assert_eq!(query.offset(), 20);
    assert_eq!(query.page(), 2);
    assert_eq!(query.filters().text.as_deref(), Some("%hello_"));
    for (page, size) in [(0, 20), (1, 0), (1, 101), (i64::MAX as u64, 100)] {
        assert!(MessageQuery::new(MessageFilters::default(), page, size).is_err());
    }
    assert!(
        MessageQuery::new(
            MessageFilters {
                webhook_config_id: Some(0),
                ..Default::default()
            },
            1,
            20
        )
        .is_err()
    );
    assert!(
        MessageQuery::new(
            MessageFilters {
                from: Some(now()),
                to: Some(now() - Duration::seconds(1)),
                ..Default::default()
            },
            1,
            20
        )
        .is_err()
    );
}

#[test]
fn summaries_preserve_whitespace_and_length_behavior() {
    assert_eq!(
        crate::pagination::summarize("  hello\n世界  "),
        "hello 世界"
    );
    assert_eq!(
        crate::pagination::summarize(&"中".repeat(200))
            .chars()
            .count(),
        30
    );
    assert_eq!(
        crate::pagination::summarize(&"中".repeat(30)),
        "中".repeat(30)
    );
    assert_eq!(
        crate::pagination::summarize(&"中".repeat(31)),
        format!("{}…", "中".repeat(29))
    );
    assert_eq!(crate::pagination::summarize(" \n\t "), "无法解析消息内容");
    assert_eq!(crate::pagination::summarize(""), "无法解析消息内容");
}
