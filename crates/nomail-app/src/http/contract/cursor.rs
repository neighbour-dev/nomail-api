use super::{MessageFilterQuery, MessageListItem};
use crate::message::{
    MessageCursorPage, MessageCursorQuery, MessageError, MessagePosition, MessageWindow,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Receipt-order pagination, not a change log. Latest/before are descending;
/// after is ascending. Keep filters unchanged when continuing a query.
#[derive(Default, Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct CursorMessagesQuery {
    /// Exclusive older boundary. Cannot be combined with after.
    pub before: Option<String>,
    /// Exclusive newer boundary. Cannot be combined with before.
    pub after: Option<String>,
    /// Default 20, range 1..=100. No page/pageSize or total count.
    #[cfg_attr(
        feature = "openapi",
        schemars(with = "Option<u64>", range(min = 1, max = 100))
    )]
    pub limit: Option<String>,
    #[serde(flatten)]
    pub filters: MessageFilterQuery,
}

// This encoding belongs to the HTTP contract, not the business/storage layers.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CursorToken {
    v: u8,
    received_at: DateTime<Utc>,
    id: Uuid,
}
fn encode(position: MessagePosition) -> String {
    URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&CursorToken {
            v: 1,
            received_at: position.received_at,
            id: position.id,
        })
        .expect("cursor serializes"),
    )
}
fn decode(value: &str) -> Result<MessagePosition, MessageError> {
    let invalid = || MessageError::InvalidInput("invalid message cursor".into());
    if value.is_empty() || value.len() > 512 {
        return Err(invalid());
    }
    let bytes = URL_SAFE_NO_PAD.decode(value).map_err(|_| invalid())?;
    let token: CursorToken = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if token.v != 1 {
        return Err(invalid());
    }
    Ok(MessagePosition {
        received_at: token.received_at,
        id: token.id,
    })
}
impl TryFrom<CursorMessagesQuery> for MessageCursorQuery {
    type Error = MessageError;
    fn try_from(q: CursorMessagesQuery) -> Result<Self, Self::Error> {
        let window = match (q.before, q.after) {
            (Some(_), Some(_)) => {
                return Err(MessageError::InvalidInput(
                    "before and after cannot be combined".into(),
                ));
            }
            (Some(value), None) => MessageWindow::Before(decode(&value)?),
            (None, Some(value)) => MessageWindow::After(decode(&value)?),
            (None, None) => MessageWindow::Latest,
        };
        let limit = q
            .limit
            .as_deref()
            .unwrap_or("20")
            .parse::<u64>()
            .map_err(|_| MessageError::InvalidInput("invalid limit".into()))?;
        Self::new(q.filters.try_into()?, window, limit)
    }
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct MessageCursorResponse {
    pub items: Vec<MessageListItem>,
    pub has_more: bool,
    /// Continue in the requested direction; null when there is no next batch.
    pub next_cursor: Option<String>,
    /// Position of the first returned item, or null for an empty result.
    pub start_cursor: Option<String>,
    /// Position of the last returned item, or null for an empty result.
    pub end_cursor: Option<String>,
}
impl From<MessageCursorPage> for MessageCursorResponse {
    fn from(page: MessageCursorPage) -> Self {
        Self {
            items: page.items.into_iter().map(Into::into).collect(),
            has_more: page.has_more,
            next_cursor: page.next.map(encode),
            start_cursor: page.start.map(encode),
            end_cursor: page.end.map(encode),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cursor_encoding_preserves_precision_and_rejects_invalid_tokens() {
        let position = MessagePosition {
            received_at: "2026-11-06T08:00:00.123456789Z".parse().unwrap(),
            id: Uuid::from_u128(7),
        };
        assert_eq!(decode(&encode(position)).unwrap(), position);
        for bad in [
            "".to_owned(),
            "?".into(),
            "a".repeat(513),
            URL_SAFE_NO_PAD.encode(b"{}"),
            URL_SAFE_NO_PAD.encode(format!(
                r#"{{"v":2,"receivedAt":"2026-01-01T00:00:00Z","id":"{}"}}"#,
                position.id
            )),
            URL_SAFE_NO_PAD.encode(format!(
                r#"{{"v":1,"receivedAt":"bad","id":"{}"}}"#,
                position.id
            )),
            URL_SAFE_NO_PAD.encode(br#"{"v":1,"receivedAt":"2026-01-01T00:00:00Z","id":"bad"}"#),
        ] {
            assert!(decode(&bad).is_err());
        }
        for query in [
            "before=x&after=y",
            "before=",
            "limit=0",
            "limit=101",
            "limit=bad",
            "limit=",
            "limit=-1",
        ] {
            let q: CursorMessagesQuery = serde_urlencoded::from_str(query).unwrap();
            assert!(MessageCursorQuery::try_from(q).is_err(), "{query}");
        }
        let q = MessageCursorQuery::try_from(CursorMessagesQuery::default()).unwrap();
        assert_eq!(q.fetch_limit(), 21);
        assert_eq!(q.window(), MessageWindow::Latest);
    }
}
