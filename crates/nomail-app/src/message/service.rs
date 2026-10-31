use super::*;
use crate::pagination::{AffectedResponse, CountResponse, Page};
use chrono::{DateTime, Utc};
use uuid::Uuid;

pub struct MessageService<S> {
    pub store: S,
}
fn missing() -> MessageError {
    MessageError::NotFound("message not found")
}
fn into_detail(row: MessageWithConfig) -> MessageDetail {
    detail(
        &row.message,
        row.message.webhook_config_id,
        &row.config_name,
    )
}
fn input(message: &str) -> MessageError {
    MessageError::InvalidInput(message.into())
}

impl<S: MessageStore> MessageService<S> {
    pub async fn list(
        &self,
        user_id: i64,
        query: ListQuery,
    ) -> MessageResult<Page<MessageListItem>> {
        let query = query.parsed()?;
        let (items, total) = self.store.list_owned(user_id, &query).await?;
        Ok(Page {
            items,
            total,
            page: query.page,
            page_size: query.page_size,
        })
    }
    pub async fn unread_count(&self, user_id: i64) -> MessageResult<CountResponse> {
        Ok(CountResponse {
            count: self.store.count_unread_owned(user_id).await?,
        })
    }
    pub async fn get(&self, user_id: i64, id: Uuid) -> MessageResult<MessageDetail> {
        self.store
            .get_owned(user_id, id)
            .await?
            .map(into_detail)
            .ok_or_else(missing)
    }
    pub async fn raw(&self, user_id: i64, id: Uuid) -> MessageResult<RawContent> {
        let mut raw = self
            .store
            .get_raw_owned(user_id, id)
            .await?
            .filter(|v| !v.text.is_empty())
            .ok_or(MessageError::NotFound("raw payload not found"))?;
        if raw.content_type.is_empty() {
            raw.content_type = "application/octet-stream".into();
        }
        Ok(raw)
    }
    pub async fn update(
        &self,
        user_id: i64,
        id: Uuid,
        request: UpdateRequest,
        now: DateTime<Utc>,
    ) -> MessageResult<MessageDetail> {
        request.validate().map_err(input)?;
        let read = request
            .status
            .as_deref()
            .map(|s| {
                ReadStatus::parse(s).map(|s| match s {
                    ReadStatus::Read => ReadChange::Read { at: now },
                    ReadStatus::Unread => ReadChange::Unread,
                })
            })
            .transpose()?;
        let patch = StatePatch {
            read,
            pinned: request.pinned,
            updated_at: now,
        };
        self.store
            .update_owned(user_id, id, &patch)
            .await?
            .map(into_detail)
            .ok_or_else(missing)
    }
    pub async fn delete(&self, user_id: i64, id: Uuid, now: DateTime<Utc>) -> MessageResult<()> {
        if self
            .store
            .mutate_many_owned(user_id, &[id], &MessageMutation::SoftDelete { at: now })
            .await?
            == 0
        {
            return Err(missing());
        }
        Ok(())
    }
    pub async fn batch(
        &self,
        user_id: i64,
        request: BatchRequest,
        now: DateTime<Utc>,
    ) -> MessageResult<AffectedResponse> {
        if request.ids.is_empty() || request.ids.len() > 100 {
            return Err(input("ids must contain 1 to 100 items"));
        }
        let mut ids = request
            .ids
            .iter()
            .map(|s| Uuid::parse_str(s).map_err(|_| input("invalid message id")))
            .collect::<MessageResult<Vec<_>>>()?;
        ids.sort_unstable();
        ids.dedup();
        let mut patch = StatePatch {
            read: None,
            pinned: None,
            updated_at: now,
        };
        let mutation = match request.action.as_str() {
            "mark_read" => {
                patch.read = Some(ReadChange::Read { at: now });
                MessageMutation::State(patch)
            }
            "mark_unread" => {
                patch.read = Some(ReadChange::Unread);
                MessageMutation::State(patch)
            }
            "pin" => {
                patch.pinned = Some(true);
                MessageMutation::State(patch)
            }
            "unpin" => {
                patch.pinned = Some(false);
                MessageMutation::State(patch)
            }
            "delete" => MessageMutation::SoftDelete { at: now },
            _ => return Err(input("unsupported batch action")),
        };
        Ok(AffectedResponse {
            affected: self
                .store
                .mutate_many_owned(user_id, &ids, &mutation)
                .await?,
        })
    }
}
