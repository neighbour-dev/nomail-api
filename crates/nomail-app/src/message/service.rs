use super::*;
use crate::pagination::Page;
use chrono::{DateTime, Utc};
use uuid::Uuid;

pub struct MessageService<S> {
    pub store: S,
}
fn missing() -> MessageError {
    MessageError::NotFound("message not found")
}

impl<S: MessageStore> MessageService<S> {
    pub async fn cursor(
        &self,
        user_id: i64,
        query: MessageCursorQuery,
    ) -> MessageResult<MessageCursorPage> {
        let items = self.store.cursor_owned(user_id, &query).await?;
        Ok(query.into_page(items))
    }
    pub async fn list(
        &self,
        user_id: i64,
        query: MessageQuery,
    ) -> MessageResult<Page<MessagePreview>> {
        let (items, total) = self.store.list_owned(user_id, &query).await?;
        Ok(Page {
            items,
            total,
            page: query.page(),
            page_size: query.page_size(),
        })
    }
    pub async fn unread_count(&self, user_id: i64) -> MessageResult<u64> {
        self.store.count_unread_owned(user_id).await
    }
    pub async fn get(&self, user_id: i64, id: Uuid) -> MessageResult<MessageView> {
        Ok(MessageView::from_stored(
            self.store
                .get_owned(user_id, id)
                .await?
                .ok_or_else(missing)?,
        ))
    }
    pub async fn raw(&self, user_id: i64, id: Uuid) -> MessageResult<RawContent> {
        self.store
            .get_raw_owned(user_id, id)
            .await?
            .ok_or(MessageError::NotFound("raw payload not found"))
    }
    pub async fn update(
        &self,
        user_id: i64,
        id: Uuid,
        request: UpdateMessage,
        now: DateTime<Utc>,
    ) -> MessageResult<MessageView> {
        let read = request.status.map(|s| match s {
            ReadStatus::Read => ReadChange::Read { at: now },
            ReadStatus::Unread => ReadChange::Unread,
        });
        let patch = StatePatch {
            read,
            pinned: request.pinned,
            updated_at: now,
        };
        let updated = self
            .store
            .update_owned(user_id, id, &patch)
            .await?
            .ok_or_else(missing)?;
        // Projection is best-effort and cannot turn a committed state update
        // into an error. Ownership/visibility remains protected by the store.
        Ok(MessageView::from_stored(updated))
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
        command: BatchMessages,
        now: DateTime<Utc>,
    ) -> MessageResult<u64> {
        let mut patch = StatePatch {
            read: None,
            pinned: None,
            updated_at: now,
        };
        let mutation = match command.action {
            BatchAction::MarkRead => {
                patch.read = Some(ReadChange::Read { at: now });
                MessageMutation::State(patch)
            }
            BatchAction::MarkUnread => {
                patch.read = Some(ReadChange::Unread);
                MessageMutation::State(patch)
            }
            BatchAction::Pin => {
                patch.pinned = Some(true);
                MessageMutation::State(patch)
            }
            BatchAction::Unpin => {
                patch.pinned = Some(false);
                MessageMutation::State(patch)
            }
            BatchAction::Delete => MessageMutation::SoftDelete { at: now },
        };
        self.store
            .mutate_many_owned(user_id, &command.ids, &mutation)
            .await
    }
}
