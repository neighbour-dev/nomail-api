use super::{
    MessagePreview, MessageQuery, MessageResult, MessageWithConfig, RawContent, ReadStatus,
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadChange {
    Read { at: DateTime<Utc> },
    Unread,
}
impl ReadChange {
    pub const fn status(self) -> ReadStatus {
        match self {
            Self::Read { .. } => ReadStatus::Read,
            Self::Unread => ReadStatus::Unread,
        }
    }
    pub const fn read_at(self) -> Option<DateTime<Utc>> {
        match self {
            Self::Read { at } => Some(at),
            Self::Unread => None,
        }
    }
}
/// Sparse state update: absent is NOT false/NULL. No immutable message fields
/// are representable, so an update cannot accidentally replace content/raw.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatePatch {
    pub read: Option<ReadChange>,
    pub pinned: Option<bool>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MessageMutation {
    State(StatePatch),
    SoftDelete { at: DateTime<Utc> },
}
impl MessageMutation {
    pub const fn updated_at(&self) -> DateTime<Utc> {
        match self {
            Self::State(p) => p.updated_at,
            Self::SoftDelete { at } => *at,
        }
    }
}

/// Scoped, atomic storage operations. Scope means an undeleted message whose
/// config is owned by the actor and undeleted (existing visibility semantics).
/// Lists must NOT load platform JSON/raw. Expiry is not an implicit filter.
#[allow(async_fn_in_trait)]
pub trait MessageStore {
    async fn list_owned(
        &self,
        user_id: i64,
        query: &MessageQuery,
    ) -> MessageResult<(Vec<MessagePreview>, u64)>;
    async fn count_unread_owned(&self, user_id: i64) -> MessageResult<u64>;
    async fn get_owned(&self, user_id: i64, id: Uuid) -> MessageResult<Option<MessageWithConfig>>;
    async fn get_raw_owned(&self, user_id: i64, id: Uuid) -> MessageResult<Option<RawContent>>;
    /// Scope check, sparse write and reading updated details must be one
    /// coherent transaction/batch. Never save a stale full Message snapshot.
    async fn update_owned(
        &self,
        user_id: i64,
        id: Uuid,
        patch: &StatePatch,
    ) -> MessageResult<Option<MessageWithConfig>>;
    /// One atomic scoped statement/transaction; absent/foreign/deleted IDs are
    /// ignored, matching rows are counted once. Must support 100 distinct IDs.
    async fn mutate_many_owned(
        &self,
        user_id: i64,
        ids: &[Uuid],
        mutation: &MessageMutation,
    ) -> MessageResult<u64>;
}
