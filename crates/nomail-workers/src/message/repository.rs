//! D1 storage adapter; no HTTP, raw query parsing, state policy or TTL decisions.
use crate::{db, time};
use chrono::{DateTime, Utc};
use nomail_app::ingestion::{IngestionError, IngestionResult, IngestionStore, ReceivingConfig};
use nomail_app::message::*;
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Clone, Copy)]
pub struct Store<'a>(pub &'a db::Store);
const OWNED: &str = "deleted_at IS NULL AND webhook_config_id IN (SELECT id FROM webhook_configs WHERE user_id=? AND deleted_at IS NULL)";
const DETAIL: &str = "SELECT notify_messages.*, (SELECT name FROM webhook_configs WHERE id=notify_messages.webhook_config_id) AS config_name FROM notify_messages";
fn storage(error: db::StorageError) -> MessageError {
    match error {
        db::StorageError::Conflict => MessageError::Conflict("resource already exists"),
        error => MessageError::Internal(error.to_string()),
    }
}
fn d1(error: worker::Error) -> MessageError {
    storage(error.into())
}
fn invalid(message: impl Into<String>) -> MessageError {
    MessageError::Internal(message.into())
}
fn date(value: DateTime<Utc>) -> String {
    time::format(value.timestamp_millis())
}
fn decode_message(mut row: Value) -> MessageResult<MessageWithConfig> {
    let name = row["config_name"]
        .as_str()
        .ok_or_else(|| invalid("missing message config name"))?
        .to_owned();
    row["pinned"] = json!(row["pinned"].as_i64() == Some(1));
    let data = row["raw_headers"]
        .as_str()
        .ok_or_else(|| invalid("missing raw headers"))?;
    let data: Value =
        serde_json::from_str(data).map_err(|e| invalid(format!("invalid raw headers: {e}")))?;
    row["raw_headers"] = data;
    row["metadata"] = json!({"event_type":row["event_type"],"version":row["event_version"],"deduplication_key":
        if row["deduplication_key"].is_null(){Value::Null}else{json!({"scope":row["deduplication_scope"],"value":row["deduplication_key"]})}});
    Ok(MessageWithConfig {
        message: serde_json::from_value(row)
            .map_err(|e| invalid(format!("invalid message storage: {e}")))?,
        config_name: name,
    })
}
#[derive(serde::Deserialize)]
struct ListRow {
    id: Uuid,
    title: String,
    summary: String,
    level: nomail_app::webhook::MessageLevel,
    status: String,
    pinned: i64,
    webhook_config_id: i64,
    created_at: DateTime<Utc>,
    expire_at: Option<DateTime<Utc>>,
}
fn decode_item(row: Value) -> MessageResult<MessagePreview> {
    let row: ListRow =
        serde_json::from_value(row).map_err(|e| invalid(format!("invalid list row: {e}")))?;
    Ok(MessagePreview {
        id: row.id,
        title: row.title,
        summary: row.summary,
        level: row.level,
        status: row.status,
        pinned: row.pinned == 1,
        webhook_config_id: row.webhook_config_id,
        received_at: row.created_at,
        expires_at: row.expire_at,
    })
}
fn filter(user_id: i64, q: &MessageFilters) -> (String, Vec<Value>) {
    let mut clause = OWNED.to_owned();
    let mut values = vec![json!(user_id)];
    if let Some(text) = &q.text {
        clause.push_str(" AND (title LIKE ? OR summary LIKE ?)");
        values.extend([json!(format!("%{text}%")), json!(format!("%{text}%"))]);
    }
    if let Some(status) = q.status {
        clause.push_str(" AND status=?");
        values.push(json!(status.as_str()));
    }
    if let Some(level) = q.level {
        clause.push_str(" AND level=?");
        values.push(json!(level));
    }
    if let Some(id) = q.webhook_config_id {
        clause.push_str(" AND webhook_config_id=?");
        values.push(json!(id.to_string()));
    }
    if let Some(pinned) = q.pinned {
        clause.push_str(" AND pinned=?");
        values.push(json!(pinned));
    }
    if let Some(from) = q.from {
        clause.push_str(" AND created_at>=?");
        values.push(json!(date(from)));
    }
    if let Some(to) = q.to {
        clause.push_str(" AND created_at<=?");
        values.push(json!(date(to)));
    }
    (clause, values)
}
/// Closed, typed mutation -> parameterized SQL. Never accepts caller-provided
/// column/action names, and never rewrites unrelated message columns.
fn mutation(command: &MessageMutation) -> (String, Vec<Value>) {
    let mut sets = vec!["updated_at=?"];
    let mut values = vec![json!(date(command.updated_at()))];
    match command {
        MessageMutation::SoftDelete { at } => {
            sets.push("deleted_at=?");
            values.push(json!(date(*at)));
        }
        MessageMutation::State(patch) => {
            if let Some(read) = patch.read {
                sets.extend(["status=?", "read_at=?"]);
                values.extend([
                    json!(read.status().as_str()),
                    json!(read.read_at().map(date)),
                ]);
            }
            if let Some(pinned) = patch.pinned {
                sets.push("pinned=?");
                values.push(json!(pinned));
            }
        }
    }
    (sets.join(","), values)
}
impl MessageStore for Store<'_> {
    async fn cursor_owned(
        &self,
        user_id: i64,
        q: &MessageCursorQuery,
    ) -> MessageResult<Vec<MessagePreview>> {
        let (mut clause, mut values) = filter(user_id, q.filters());
        let order = if q.window().ascending() {
            "ASC"
        } else {
            "DESC"
        };
        match q.window() {
            MessageWindow::Before(position) | MessageWindow::After(position) => {
                // D1 stores fixed millisecond UTC times. A boundary between
                // milliseconds falls strictly after every ID in its floor ms.
                if position
                    .received_at
                    .timestamp_subsec_nanos()
                    .is_multiple_of(1_000_000)
                {
                    clause.push_str(if q.window().ascending() {
                        " AND (created_at,id)>(?,?)"
                    } else {
                        " AND (created_at,id)<(?,?)"
                    });
                    values.extend([json!(date(position.received_at)), json!(position.id)]);
                } else {
                    clause.push_str(if q.window().ascending() {
                        " AND created_at>?"
                    } else {
                        " AND created_at<=?"
                    });
                    values.push(json!(date(position.received_at)));
                }
            }
            MessageWindow::Latest => (),
        }
        values.push(json!(q.fetch_limit()));
        self.0.rows(&format!("SELECT id,title,summary,level,status,pinned,webhook_config_id,created_at,expire_at FROM notify_messages WHERE {clause} ORDER BY created_at {order},id {order} LIMIT ?"), values)
            .await.map_err(storage)?.into_iter().map(decode_item).collect()
    }
    async fn list_owned(
        &self,
        user_id: i64,
        q: &MessageQuery,
    ) -> MessageResult<(Vec<MessagePreview>, u64)> {
        let (clause, values) = filter(user_id, q.filters());
        let mut page_values = values.clone();
        // Bind OFFSET as text so SQLite can convert its exact i64 value without
        // an intermediate JS Number losing integer precision.
        page_values.extend([json!(q.page_size()), json!(q.offset().to_string())]);
        let result=self.0.0.batch(vec![
            self.0.statement(&format!("SELECT COUNT(*) AS total FROM notify_messages WHERE {clause}"),values).map_err(storage)?,
            self.0.statement(&format!("SELECT id,title,summary,level,status,pinned,webhook_config_id,created_at,expire_at FROM notify_messages WHERE {clause} ORDER BY created_at DESC LIMIT ? OFFSET ?"),page_values).map_err(storage)?,
        ]).await.map_err(d1)?;
        let counts = result[0].results::<Value>().map_err(d1)?;
        let total = counts
            .first()
            .and_then(|v| v["total"].as_u64())
            .ok_or_else(|| invalid("invalid row count"))?;
        let items = result[1]
            .results::<Value>()
            .map_err(d1)?
            .into_iter()
            .map(decode_item)
            .collect::<MessageResult<Vec<_>>>()?;
        Ok((items, total))
    }
    async fn count_unread_owned(&self, user_id: i64) -> MessageResult<u64> {
        let rows = self
            .0
            .rows(
                &format!(
                    "SELECT COUNT(*) AS count FROM notify_messages WHERE {OWNED} AND status=?"
                ),
                vec![json!(user_id), json!(ReadStatus::Unread.as_str())],
            )
            .await
            .map_err(storage)?;
        rows.first()
            .and_then(|v| v["count"].as_u64())
            .ok_or_else(|| invalid("invalid unread count"))
    }
    async fn get_owned(&self, user_id: i64, id: Uuid) -> MessageResult<Option<MessageWithConfig>> {
        self.0
            .rows(
                &format!("{DETAIL} WHERE {OWNED} AND id=?"),
                vec![json!(user_id), json!(id)],
            )
            .await
            .map_err(storage)?
            .into_iter()
            .next()
            .map(decode_message)
            .transpose()
    }
    async fn get_raw_owned(&self, user_id: i64, id: Uuid) -> MessageResult<Option<RawContent>> {
        self.0
            .rows(
                &format!(
                    "SELECT raw_headers,raw_payload,content_type FROM notify_messages WHERE {OWNED} AND id=?"
                ),
                vec![json!(user_id), json!(id)],
            )
            .await
            .map_err(storage)?
            .into_iter()
            .next()
            .map(|row| {
                Ok(RawContent {
                    header: serde_json::from_str(row["raw_headers"].as_str().ok_or_else(|| invalid("missing raw headers"))?)
                        .map_err(|e| invalid(format!("invalid raw headers: {e}")))?,
                    text: row["raw_payload"]
                        .as_str()
                        .ok_or_else(|| invalid("invalid raw payload"))?
                        .into(),
                    content_type: row["content_type"]
                        .as_str()
                        .ok_or_else(|| invalid("invalid raw content type"))?
                        .into(),
                })
            })
            .transpose()
    }
    async fn update_owned(
        &self,
        user_id: i64,
        id: Uuid,
        patch: &StatePatch,
    ) -> MessageResult<Option<MessageWithConfig>> {
        let (sets, mut values) = mutation(&MessageMutation::State(patch.clone()));
        values.extend([json!(user_id), json!(id)]);
        let result = self
            .0
            .0
            .batch(vec![
                self.0
                    .statement(
                        &format!(
                            "UPDATE notify_messages SET {sets} WHERE {OWNED} AND id=? RETURNING id"
                        ),
                        values,
                    )
                    .map_err(storage)?,
                self.0
                    .statement(
                        &format!("{DETAIL} WHERE {OWNED} AND id=? AND changes()>0"),
                        vec![json!(user_id), json!(id)],
                    )
                    .map_err(storage)?,
            ])
            .await
            .map_err(d1)?;
        if result[0].results::<Value>().map_err(d1)?.is_empty() {
            return Ok(None);
        }
        let row = result[1]
            .results::<Value>()
            .map_err(d1)?
            .into_iter()
            .next()
            .ok_or_else(|| invalid("updated message disappeared"))?;
        Ok(Some(decode_message(row)?))
    }
    async fn mutate_many_owned(
        &self,
        user_id: i64,
        ids: &[Uuid],
        command: &MessageMutation,
    ) -> MessageResult<u64> {
        if ids.is_empty() {
            return Ok(0);
        }
        let (sets, mut values) = mutation(command);
        // D1 limits bound parameter count. A JSON ID array keeps the promised
        // 100-item operation atomic without chunking or exceeding that limit.
        let ids = serde_json::to_string(ids).map_err(|e| invalid(e.to_string()))?;
        values.extend([json!(user_id), json!(ids)]);
        let rows=self.0.rows(&format!("UPDATE notify_messages SET {sets} WHERE {OWNED} AND id IN (SELECT value FROM json_each(?)) RETURNING id"),values).await.map_err(storage)?;
        Ok(rows.len() as u64)
    }
}
impl IngestionStore for Store<'_> {
    async fn receiving_config(&self, key: &str) -> IngestionResult<Option<ReceivingConfig>> {
        self.0.rows(
            "SELECT id,user_id,provider,encrypted_platform FROM webhook_configs WHERE key=? AND enabled=1 AND deleted_at IS NULL",
            vec![json!(key)],
        ).await.map_err(|e| IngestionError::Internal(e.to_string()))?
            .into_iter().next().map(|row| {
                let invalid = || IngestionError::Internal("invalid receiving configuration".into());
                Ok(ReceivingConfig {
                    id: row["id"].as_i64().ok_or_else(invalid)?,
                    user_id: row["user_id"].as_i64().ok_or_else(invalid)?,
                    provider: row["provider"].as_str().ok_or_else(invalid)?.parse().map_err(|_| invalid())?,
                    encrypted_platform: row["encrypted_platform"].as_str().ok_or_else(|| IngestionError::Internal("missing platform configuration".into()))?.to_owned(),
                })
            }).transpose()
    }
    async fn insert_once(&self, m: &Message) -> MessageResult<InsertOutcome> {
        let key = m.metadata.deduplication_key.as_ref();
        let data = serde_json::to_string(&m.raw_headers).map_err(|e| invalid(e.to_string()))?;
        let rows=self.0.rows("INSERT INTO notify_messages (id,webhook_config_id,provider,event_type,event_version,deduplication_scope,deduplication_key,raw_headers,title,summary,level,url,raw_payload,content_type,created_at,updated_at,expire_at,status,pinned,read_at,deleted_at) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(webhook_config_id,deduplication_scope,deduplication_key) DO NOTHING RETURNING id",
            vec![json!(m.id),json!(m.webhook_config_id),json!(m.provider),json!(m.metadata.event_type),json!(m.metadata.version),
                 json!(key.map(|k|&k.scope)),json!(key.map(|k|&k.value)),json!(data),json!(m.common.title),json!(m.common.summary),
                 json!(m.common.level),json!(m.common.url),json!(m.raw_payload),json!(m.content_type),
                json!(date(m.received_at)),json!(date(m.updated_at)),json!(m.expires_at.map(date)),json!(m.status),json!(m.pinned),json!(m.read_at.map(date)),json!(m.deleted_at.map(date))]).await.map_err(storage)?;
        if !rows.is_empty() {
            return Ok(InsertOutcome::Created(m.id));
        }
        let key = key.ok_or_else(|| invalid("insert without a deduplication key was ignored"))?;
        let previous=self.0.rows("SELECT id FROM notify_messages WHERE webhook_config_id=? AND deduplication_scope=? AND deduplication_key=?",
            vec![json!(m.webhook_config_id),json!(key.scope),json!(key.value)]).await.map_err(storage)?;
        let id = previous
            .first()
            .and_then(|v| v["id"].as_str())
            .and_then(|v| Uuid::parse_str(v).ok())
            .ok_or_else(|| invalid("duplicate message disappeared"))?;
        Ok(InsertOutcome::Duplicate(id))
    }
}
