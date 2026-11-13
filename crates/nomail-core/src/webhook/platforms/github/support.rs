//! GitHub-local native extraction. Never coerce strings or reindex arrays.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub(super) trait GithubValue: Sized {
    fn read(value: &Value) -> Option<Self>;
}

macro_rules! primitive {
    ($ty:ty, $method:ident) => {
        impl GithubValue for $ty {
            fn read(value: &Value) -> Option<Self> {
                value.$method()
            }
        }
    };
}
primitive!(bool, as_bool);
primitive!(u64, as_u64);
primitive!(i64, as_i64);

impl GithubValue for String {
    fn read(value: &Value) -> Option<Self> {
        value.as_str().map(str::to_owned)
    }
}
impl GithubValue for Value {
    fn read(value: &Value) -> Option<Self> {
        Some(value.clone())
    }
}
impl<T: GithubValue> GithubValue for Box<T> {
    fn read(value: &Value) -> Option<Self> {
        T::read(value).map(Box::new)
    }
}
impl<T: GithubValue> GithubValue for Option<T> {
    fn read(value: &Value) -> Option<Self> {
        if value.is_null() {
            Some(None)
        } else {
            T::read(value).map(Some)
        }
    }
}
impl<T: GithubValue> GithubValue for Vec<T> {
    fn read(value: &Value) -> Option<Self> {
        value.as_array()?.iter().map(T::read).collect()
    }
}
impl<T: GithubValue> GithubValue for BTreeMap<String, T> {
    fn read(value: &Value) -> Option<Self> {
        value
            .as_object()?
            .iter()
            .map(|(key, value)| Some((key.clone(), T::read(value)?)))
            .collect()
    }
}

/// A native JSON number without another conversion through f64.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct GithubNumber(
    #[cfg_attr(feature = "openapi", schemars(with = "f64"))] pub serde_json::Number,
);
impl GithubValue for GithubNumber {
    fn read(value: &Value) -> Option<Self> {
        value.as_number().cloned().map(Self)
    }
}

macro_rules! github_model {
    ($name:ident { $($field:ident: $ty:ty = $key:literal),* $(,)? }) => {
        #[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
        #[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
        pub struct $name {
            $(#[serde(rename = $key, default, skip_serializing_if = "Option::is_none",
                deserialize_with = "crate::webhook::platforms::native::deserialize_present")]
            pub $field: crate::webhook::platforms::native::JsonField<$ty>,)*
        }
        impl $crate::webhook::platforms::github::support::GithubValue for $name {
            fn read(value: &serde_json::Value) -> Option<Self> {
                value.as_object()?;
                Some(Self { $($field: crate::webhook::platforms::native::read_json_field(value, $key,
                    <$ty as $crate::webhook::platforms::github::support::GithubValue>::read),)* })
            }
        }
    };
}

// Alternatives are selected by JSON kind, not by first-match optional structs.
// Object alternatives without a reliable discriminator use a merged projection.
macro_rules! github_union {
    ($name:ident { $($variant:ident($ty:ty) if $condition:expr),+ $(,)? }) => {
        #[derive(Debug, Clone, PartialEq, serde::Serialize)]
        #[serde(untagged)]
        #[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
        pub enum $name { $($variant($ty),)+ }
        impl $crate::webhook::platforms::github::support::GithubValue for $name {
            fn read(value: &serde_json::Value) -> Option<Self> {
                $(if ($condition)(value) {
                    return <$ty as $crate::webhook::platforms::github::support::GithubValue>::read(value).map(Self::$variant);
                })+
                None
            }
        }
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = <serde_json::Value as serde::Deserialize>::deserialize(deserializer)?;
                <Self as $crate::webhook::platforms::github::support::GithubValue>::read(&value)
                    .ok_or_else(|| serde::de::Error::custom("invalid GitHub native field type"))
            }
        }
    };
}
