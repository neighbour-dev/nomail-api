//! Typed Telegram Bot API webhook payload models, preserving native nesting.
//!
//! Field definitions follow https://core.telegram.org/bots/api (2026-11-10).
//! These are tolerant projections, not outbound API request validators: even
//! required nested fields are optional here so a malformed field does not hide
//! valid siblings. Unknown fields remain in the event's complete `raw` payload.
//! Strings are verbatim, including whitespace and UTF-16 entity offsets.
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(super) trait TelegramValue: Sized {
    fn read(value: &Value) -> Option<Self>;
}

impl TelegramValue for String {
    fn read(value: &Value) -> Option<Self> {
        value.as_str().map(str::to_owned)
    }
}

impl TelegramValue for i64 {
    fn read(value: &Value) -> Option<Self> {
        value.as_i64()
    }
}

impl TelegramValue for f64 {
    fn read(value: &Value) -> Option<Self> {
        value.as_f64()
    }
}

impl TelegramValue for bool {
    fn read(value: &Value) -> Option<Self> {
        value.as_bool()
    }
}

impl<T: TelegramValue> TelegramValue for Box<T> {
    fn read(value: &Value) -> Option<Self> {
        T::read(value).map(Box::new)
    }
}

impl<T: TelegramValue> TelegramValue for Vec<T> {
    fn read(value: &Value) -> Option<Self> {
        Some(value.as_array()?.iter().filter_map(T::read).collect())
    }
}

macro_rules! model {
    ($(#[$attr:meta])* $name:ident $( [$required:ident: $required_ty:ty] )? { $($field:ident: $ty:ty = $key:literal),* $(,)? }) => {
        $(#[$attr])*
        #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
        #[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
        pub struct $name {
            $(pub $required: $required_ty,)?
            $(#[serde(rename = $key, skip_serializing_if = "Option::is_none")]
            pub $field: Option<$ty>,)*
        }

        impl TelegramValue for $name {
            fn read(value: &Value) -> Option<Self> {
                value.as_object()?;
                Some(Self {
                    $($required: <$required_ty>::read(&value[stringify!($required)])?,)?
                    $($field: <$ty>::read(&value[$key]),)*
                })
            }
        }
    };
}

// Select by the documented discriminator, never by trial-deserializing
// defaulted structs (which would incorrectly match the first union member).
macro_rules! union {
    ($(#[$attr:meta])* $name:ident, $key:literal, { $($variant:ident($ty:ty) = $tag:literal),* $(,)? }) => {
        $(#[$attr])*
        #[derive(Debug, Clone, PartialEq, Serialize)]
        #[serde(untagged)]
        #[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
        pub enum $name {
            $($variant(Box<$ty>),)*
            /// A future object variant, retained without coercion.
            Unknown(serde_json::Map<String, Value>),
        }

        impl TelegramValue for $name {
            fn read(value: &Value) -> Option<Self> {
                let object = value.as_object()?;
                match value[$key].as_str() {
                    $(Some($tag) => <$ty>::read(value).map(|v| Self::$variant(Box::new(v))),)*
                    _ => Some(Self::Unknown(object.clone())),
                }
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = Value::deserialize(deserializer)?;
                Self::read(&value).ok_or_else(|| serde::de::Error::custom("expected Telegram object"))
            }
        }
    };
}

mod types;
pub use types::*;

pub(super) fn is_update_field(field: &str) -> bool {
    types::UPDATE_FIELDS.contains(&field)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub enum TelegramMaybeInaccessibleMessage {
    Message(Box<TelegramMessage>),
    InaccessibleMessage(Box<TelegramInaccessibleMessage>),
    Unknown(serde_json::Map<String, Value>),
}

impl TelegramValue for TelegramMaybeInaccessibleMessage {
    fn read(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        match value["date"].as_i64() {
            Some(0) => TelegramInaccessibleMessage::read(value)
                .map(|v| Self::InaccessibleMessage(Box::new(v))),
            Some(date) if date > 0 => {
                TelegramMessage::read(value).map(|v| Self::Message(Box::new(v)))
            }
            _ => Some(Self::Unknown(object.clone())),
        }
    }
}

impl<'de> Deserialize<'de> for TelegramMaybeInaccessibleMessage {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        Self::read(&value).ok_or_else(|| serde::de::Error::custom("expected Telegram message"))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub enum TelegramRichText {
    Text(String),
    Array(Vec<TelegramRichText>),
    Formatted(TelegramFormattedRichText),
}

impl TelegramValue for TelegramRichText {
    fn read(value: &Value) -> Option<Self> {
        match value {
            Value::String(text) => Some(Self::Text(text.clone())),
            Value::Array(_) => Vec::read(value).map(Self::Array),
            Value::Object(_) => TelegramFormattedRichText::read(value).map(Self::Formatted),
            _ => None,
        }
    }
}

impl<'de> Deserialize<'de> for TelegramRichText {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        Self::read(&value).ok_or_else(|| serde::de::Error::custom("expected Telegram rich text"))
    }
}
