//! SQLite storage adapter. Policies/defaults/state transitions are App use cases.
use crate::{
    db,
    entity::{message, webhook},
};
use chrono::{DateTime, Utc};
use nomail_app::ingestion::{IngestionError, IngestionResult, IngestionStore, ReceivingConfig};
use nomail_app::message::*;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait, FromQueryResult,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, QueryTrait, Select, Set,
    TransactionTrait, UpdateMany,
    sea_query::{Expr, OnConflict, Query},
};
use uuid::Uuid;

#[derive(Clone, Copy)]
pub struct Store<'a>(pub &'a DatabaseConnection);
fn storage(error: DbErr) -> MessageError {
    if matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    ) {
        MessageError::Conflict("resource already exists")
    } else {
        MessageError::Internal(error.to_string())
    }
}
fn conversion(error: serde_json::Error) -> MessageError {
    MessageError::Internal(error.to_string())
}

#[derive(FromQueryResult)]
struct ListRow {
    id: Uuid,
    title: String,
    content: String,
    source: Option<String>,
    level: String,
    status: String,
    pinned: bool,
    webhook_config_id: i64,
    created_at: DateTime<Utc>,
    expire_at: Option<DateTime<Utc>>,
}
impl TryFrom<ListRow> for MessagePreview {
    type Error = MessageError;
    fn try_from(m: ListRow) -> MessageResult<Self> {
        Ok(Self {
            id: m.id,
            title: m.title,
            summary: nomail_app::pagination::summarize(&m.content),
            source: m.source,
            level: serde_json::from_value(serde_json::json!(m.level)).map_err(conversion)?,
            status: m.status,
            pinned: m.pinned,
            webhook_config_id: m.webhook_config_id,
            received_at: m.created_at,
            expires_at: m.expire_at,
        })
    }
}
#[derive(FromQueryResult)]
struct RawRow {
    raw_payload: String,
    content_type: String,
}

fn owned(user_id: i64) -> Select<message::Entity> {
    let configs = Query::select()
        .column(webhook::Column::Id)
        .from(webhook::Entity)
        .and_where(webhook::Column::UserId.eq(user_id))
        .and_where(webhook::Column::DeletedAt.is_null())
        .to_owned();
    message::Entity::find()
        .filter(message::Column::DeletedAt.is_null())
        .filter(Expr::col(message::Column::WebhookConfigId).in_subquery(configs))
}
fn owned_ids(user_id: i64) -> sea_orm::sea_query::SelectStatement {
    owned(user_id)
        .select_only()
        .column(message::Column::Id)
        .into_query()
}
fn filtered(user_id: i64, q: &MessageQuery) -> Select<message::Entity> {
    let q = q.filters();
    let mut query = owned(user_id);
    if let Some(text) = &q.text {
        // Preserve existing substring/wildcard search semantics; no new search engine.
        query = query.filter(
            Expr::col(message::Column::Title)
                .like(format!("%{text}%"))
                .or(Expr::col(message::Column::Content).like(format!("%{text}%"))),
        );
    }
    if let Some(status) = q.status {
        query = query.filter(message::Column::Status.eq(status.as_str()));
    }
    if let Some(level) = q.level {
        query = query.filter(message::Column::Level.eq(level.as_str()));
    }
    if let Some(id) = q.webhook_config_id {
        query = query.filter(message::Column::WebhookConfigId.eq(id));
    }
    if let Some(pinned) = q.pinned {
        query = query.filter(message::Column::Pinned.eq(pinned));
    }
    if let Some(from) = q.from {
        query = query.filter(message::Column::CreatedAt.gte(from));
    }
    if let Some(to) = q.to {
        query = query.filter(message::Column::CreatedAt.lte(to));
    }
    query
}
async fn detail(
    db: &impl ConnectionTrait,
    user_id: i64,
    id: Uuid,
) -> MessageResult<Option<MessageWithConfig>> {
    let Some(row) = owned(user_id)
        .filter(message::Column::Id.eq(id))
        .one(db)
        .await
        .map_err(storage)?
    else {
        return Ok(None);
    };
    let config = webhook::Entity::find_by_id(row.webhook_config_id)
        .filter(webhook::Column::UserId.eq(user_id))
        .filter(webhook::Column::DeletedAt.is_null())
        .one(db)
        .await
        .map_err(storage)?;
    let Some(config) = config else {
        return Ok(None);
    };
    Ok(Some(MessageWithConfig {
        message: (&row).try_into().map_err(conversion)?,
        config_name: config.name,
    }))
}
/// Only encoding of a closed, typed mutation. No raw user action/column names.
fn mutation(command: &MessageMutation) -> UpdateMany<message::Entity> {
    let mut update = message::Entity::update_many().col_expr(
        message::Column::UpdatedAt,
        Expr::value(command.updated_at()),
    );
    match command {
        MessageMutation::SoftDelete { at } => {
            update = update.col_expr(message::Column::DeletedAt, Expr::value(*at))
        }
        MessageMutation::State(patch) => {
            if let Some(read) = patch.read {
                update = update
                    .col_expr(message::Column::Status, Expr::value(read.status().as_str()))
                    .col_expr(message::Column::ReadAt, Expr::value(read.read_at()));
            }
            if let Some(pinned) = patch.pinned {
                update = update.col_expr(message::Column::Pinned, Expr::value(pinned));
            }
        }
    }
    update
}
impl MessageStore for Store<'_> {
    async fn list_owned(
        &self,
        user_id: i64,
        q: &MessageQuery,
    ) -> MessageResult<(Vec<MessagePreview>, u64)> {
        let tx = self.0.begin().await.map_err(storage)?;
        let query = filtered(user_id, q);
        let total = query.clone().count(&tx).await.map_err(storage)?;
        let rows = query
            .select_only()
            .columns([
                message::Column::Id,
                message::Column::Title,
                message::Column::Content,
                message::Column::Source,
                message::Column::Level,
                message::Column::Status,
                message::Column::Pinned,
                message::Column::WebhookConfigId,
                message::Column::CreatedAt,
                message::Column::ExpireAt,
            ])
            .order_by_desc(message::Column::CreatedAt)
            .offset(q.offset())
            .limit(q.page_size())
            .into_model::<ListRow>()
            .all(&tx)
            .await
            .map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok((
            rows.into_iter()
                .map(TryInto::try_into)
                .collect::<MessageResult<Vec<_>>>()?,
            total,
        ))
    }
    async fn count_unread_owned(&self, user_id: i64) -> MessageResult<u64> {
        owned(user_id)
            .filter(message::Column::Status.eq(ReadStatus::Unread.as_str()))
            .count(self.0)
            .await
            .map_err(storage)
    }
    async fn get_owned(&self, user_id: i64, id: Uuid) -> MessageResult<Option<MessageWithConfig>> {
        let tx = self.0.begin().await.map_err(storage)?;
        let result = detail(&tx, user_id, id).await?;
        tx.commit().await.map_err(storage)?;
        Ok(result)
    }
    async fn get_raw_owned(&self, user_id: i64, id: Uuid) -> MessageResult<Option<RawContent>> {
        Ok(owned(user_id)
            .filter(message::Column::Id.eq(id))
            .select_only()
            .columns([message::Column::RawPayload, message::Column::ContentType])
            .into_model::<RawRow>()
            .one(self.0)
            .await
            .map_err(storage)?
            .map(|row| RawContent {
                text: row.raw_payload,
                content_type: row.content_type,
            }))
    }
    async fn update_owned(
        &self,
        user_id: i64,
        id: Uuid,
        patch: &StatePatch,
    ) -> MessageResult<Option<MessageWithConfig>> {
        let tx = db::write_transaction(self.0).await.map_err(storage)?;
        let affected = mutation(&MessageMutation::State(patch.clone()))
            .filter(message::Column::Id.eq(id))
            .filter(Expr::col(message::Column::Id).in_subquery(owned_ids(user_id)))
            .exec(&tx)
            .await
            .map_err(storage)?
            .rows_affected;
        let result = if affected == 0 {
            None
        } else {
            detail(&tx, user_id, id).await?
        };
        tx.commit().await.map_err(storage)?;
        Ok(result)
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
        Ok(mutation(command)
            .filter(message::Column::Id.is_in(ids.iter().copied()))
            .filter(Expr::col(message::Column::Id).in_subquery(owned_ids(user_id)))
            .exec(self.0)
            .await
            .map_err(storage)?
            .rows_affected)
    }
}
impl IngestionStore for Store<'_> {
    async fn receiving_config(&self, key: &str) -> IngestionResult<Option<ReceivingConfig>> {
        webhook::Entity::find()
            .filter(webhook::Column::Key.eq(key))
            .filter(webhook::Column::Enabled.eq(true))
            .filter(webhook::Column::DeletedAt.is_null())
            .one(self.0)
            .await
            .map_err(|e| IngestionError::Internal(e.to_string()))?
            .map(|row| {
                Ok(ReceivingConfig {
                    id: row.id,
                    user_id: row.user_id,
                    provider: row
                        .provider
                        .parse()
                        .map_err(|_| IngestionError::Internal("invalid stored provider".into()))?,
                    source: row.source,
                    encrypted_secret: row.encrypted_secret,
                })
            })
            .transpose()
    }
    async fn insert_once(&self, m: &Message) -> MessageResult<InsertOutcome> {
        let key = m.metadata.deduplication_key.as_ref();
        let record = message::ActiveModel {
            id: Set(m.id),
            webhook_config_id: Set(m.webhook_config_id),
            created_at: Set(m.received_at),
            updated_at: Set(m.updated_at),
            deleted_at: Set(m.deleted_at),
            provider: Set(m.provider.as_str().into()),
            event_type: Set(m.metadata.event_type.clone()),
            event_version: Set(m.metadata.version.clone()),
            deduplication_scope: Set(key.map(|k| k.scope.clone())),
            deduplication_key: Set(key.map(|k| k.value.clone())),
            platform_data: Set(serde_json::to_string(&m.platform_data).map_err(conversion)?),
            title: Set(m.common.title.clone()),
            content: Set(m.common.content.clone()),
            source: Set(m.common.source.clone()),
            level: Set(m.common.level.as_str().into()),
            url: Set(m.common.url.clone()),
            raw_payload: Set(m.raw_payload.clone()),
            content_type: Set(m.content_type.clone()),
            status: Set(m.status.clone()),
            pinned: Set(m.pinned),
            read_at: Set(m.read_at),
            expire_at: Set(m.expires_at),
        };
        let affected = message::Entity::insert(record)
            .on_conflict(
                OnConflict::columns([
                    message::Column::WebhookConfigId,
                    message::Column::DeduplicationScope,
                    message::Column::DeduplicationKey,
                ])
                .do_nothing()
                .to_owned(),
            )
            .exec_without_returning(self.0)
            .await
            .map_err(storage)?;
        if affected != 0 {
            return Ok(InsertOutcome::Created(m.id));
        }
        let key = key.ok_or_else(|| {
            MessageError::Internal("insert without a deduplication key was ignored".into())
        })?;
        let previous = message::Entity::find()
            .filter(message::Column::WebhookConfigId.eq(m.webhook_config_id))
            .filter(message::Column::DeduplicationScope.eq(&key.scope))
            .filter(message::Column::DeduplicationKey.eq(&key.value))
            .one(self.0)
            .await
            .map_err(storage)?
            .ok_or_else(|| MessageError::Internal("duplicate message disappeared".into()))?;
        Ok(InsertOutcome::Duplicate(previous.id))
    }
}
