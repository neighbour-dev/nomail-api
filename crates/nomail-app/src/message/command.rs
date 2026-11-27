use super::{MessageError, MessageResult};
use garde::Validate;
use std::str::FromStr;
use uuid::Uuid;

pub struct UpdateMessage {
    pub(super) status: Option<super::ReadStatus>,
    pub(super) pinned: Option<bool>,
}
impl UpdateMessage {
    pub fn new(status: Option<super::ReadStatus>, pinned: Option<bool>) -> MessageResult<Self> {
        if status.is_none() && pinned.is_none() {
            return Err(MessageError::InvalidInput(
                "status or pinned is required".into(),
            ));
        }
        Ok(Self { status, pinned })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchAction {
    MarkRead,
    MarkUnread,
    Pin,
    Unpin,
    Delete,
}
impl FromStr for BatchAction {
    type Err = MessageError;
    fn from_str(value: &str) -> MessageResult<Self> {
        match value {
            "mark_read" => Ok(Self::MarkRead),
            "mark_unread" => Ok(Self::MarkUnread),
            "pin" => Ok(Self::Pin),
            "unpin" => Ok(Self::Unpin),
            "delete" => Ok(Self::Delete),
            _ => Err(MessageError::InvalidInput(
                "unsupported batch action".into(),
            )),
        }
    }
}

pub struct BatchMessages {
    pub(super) ids: Vec<Uuid>,
    pub(super) action: BatchAction,
}

#[derive(Validate)]
#[garde(transparent)]
struct BatchIds<'a, T>(#[garde(length(min = 1, max = 100))] &'a [T]);

impl<T> BatchIds<'_, T> {
    fn validate_count(&self) -> MessageResult<()> {
        self.validate()
            .map_err(|_| MessageError::InvalidInput("ids must contain 1 to 100 items".into()))
    }
}

impl BatchMessages {
    fn build(mut ids: Vec<Uuid>, action: BatchAction) -> Self {
        ids.sort_unstable();
        ids.dedup();
        Self { ids, action }
    }
    pub fn new(ids: Vec<Uuid>, action: BatchAction) -> MessageResult<Self> {
        BatchIds(&ids).validate_count()?;
        Ok(Self::build(ids, action))
    }
    /// Preserve input error order: count before UUIDs, UUIDs before action.
    pub fn parse(ids: Vec<String>, action: &str) -> MessageResult<Self> {
        BatchIds(&ids).validate_count()?;
        let ids = ids
            .into_iter()
            .map(|id| {
                Uuid::parse_str(&id)
                    .map_err(|_| MessageError::InvalidInput("invalid message id".into()))
            })
            .collect::<MessageResult<Vec<_>>>()?;
        Ok(Self::build(ids, action.parse()?))
    }
    pub fn ids(&self) -> &[Uuid] {
        &self.ids
    }
    pub fn action(&self) -> BatchAction {
        self.action
    }
}
