use super::{MessageError, MessageResult};
use garde::Validate;
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
pub struct BatchMessages {
    pub(super) ids: Vec<Uuid>,
    pub(super) action: BatchAction,
}

#[derive(Validate)]
#[garde(transparent)]
struct BatchIds<'a, T>(#[garde(length(min = 1, max = 100))] &'a [T]);

impl BatchMessages {
    pub(crate) fn validate_count<T>(ids: &[T]) -> MessageResult<()> {
        BatchIds(ids)
            .validate()
            .map_err(|_| MessageError::InvalidInput("ids must contain 1 to 100 items".into()))
    }

    pub fn new(mut ids: Vec<Uuid>, action: BatchAction) -> MessageResult<Self> {
        Self::validate_count(&ids)?;
        ids.sort_unstable();
        ids.dedup();
        Ok(Self { ids, action })
    }
    pub fn ids(&self) -> &[Uuid] {
        &self.ids
    }
    pub fn action(&self) -> BatchAction {
        self.action
    }
}
