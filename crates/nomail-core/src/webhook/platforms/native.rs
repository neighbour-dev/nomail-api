//! Field-local extraction for native payloads. No string coercion or trimming.
use serde_json::Value;

/// JSON property presence: None = missing, Some(None) = explicit null,
/// Some(Some(value)) = a supplied value (including empty collections/strings).
pub type JsonField<T> = Option<Option<T>>;

/// Convenience accessors for mappings that intentionally treat missing/null as
/// no typed value. Serialization uses the original two Option layers instead.
pub trait JsonFieldExt<T> {
    fn value(&self) -> Option<&T>;
    fn into_value(self) -> Option<T>;
    fn value_deref(&self) -> Option<&T::Target>
    where
        T: std::ops::Deref;
}

impl<T> JsonFieldExt<T> for JsonField<T> {
    fn value(&self) -> Option<&T> {
        self.as_ref().and_then(Option::as_ref)
    }
    fn into_value(self) -> Option<T> {
        self.flatten()
    }
    fn value_deref(&self) -> Option<&T::Target>
    where
        T: std::ops::Deref,
    {
        self.value().map(std::ops::Deref::deref)
    }
}

pub(super) fn deserialize_present<'de, D, T>(deserializer: D) -> Result<JsonField<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    <Option<T> as serde::Deserialize>::deserialize(deserializer).map(Some)
}

pub(super) fn read_json_field<T>(
    object: &Value,
    key: &str,
    read: impl FnOnce(&Value) -> Option<T>,
) -> JsonField<T> {
    match object.get(key) {
        None => None,
        Some(Value::Null) => Some(None),
        Some(value) => read(value).map(Some),
    }
}

macro_rules! payload_field_type {
    (Option<$ty:ty>) => { crate::webhook::platforms::native::JsonField<$ty> };
    (Vec<$ty:ty>) => { crate::webhook::platforms::native::JsonField<Vec<$ty>> };
    (BTreeMap<$key:ty, $value:ty>) => { crate::webhook::platforms::native::JsonField<BTreeMap<$key, $value>> };
}
macro_rules! payload_field_key {
    ($name:ident) => {
        stringify!($name)
    };
    ($name:ident, $key:literal) => {
        $key
    };
}

pub(super) trait NativeValue: Sized {
    fn read(value: &Value) -> Option<Self>;
}
impl NativeValue for String {
    fn read(value: &Value) -> Option<Self> {
        value.as_str().map(str::to_owned)
    }
}
impl NativeValue for bool {
    fn read(value: &Value) -> Option<Self> {
        value.as_bool()
    }
}
impl NativeValue for i64 {
    fn read(value: &Value) -> Option<Self> {
        value.as_i64()
    }
}
impl NativeValue for u64 {
    fn read(value: &Value) -> Option<Self> {
        value.as_u64()
    }
}
impl NativeValue for Value {
    fn read(value: &Value) -> Option<Self> {
        Some(value.clone())
    }
}
impl<T: NativeValue> NativeValue for Vec<T> {
    fn read(value: &Value) -> Option<Self> {
        Some(value.as_array()?.iter().filter_map(T::read).collect())
    }
}

macro_rules! native_model {
    ($name:ident { $($field:ident: $ty:ty = $key:literal),* $(,)? }) => {
        #[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
        #[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
        pub struct $name {
            $(#[serde(rename = $key, default, skip_serializing_if = "Option::is_none",
                deserialize_with = "crate::webhook::platforms::native::deserialize_present")]
            pub $field: crate::webhook::platforms::native::JsonField<$ty>,)*
        }
        impl crate::webhook::platforms::native::NativeValue for $name {
            fn read(value: &serde_json::Value) -> Option<Self> {
                value.as_object()?;
                Some(Self { $($field: crate::webhook::platforms::native::read_json_field(value, $key,
                    <$ty as crate::webhook::platforms::native::NativeValue>::read),)* })
            }
        }
    };
}

#[cfg(test)]
mod presence_tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Probe {
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_present"
        )]
        value: JsonField<Value>,
    }

    #[test]
    fn standard_options_and_serde_adapter_preserve_all_three_states() {
        let absent: Probe = serde_json::from_value(json!({})).unwrap();
        assert_eq!(absent.value, None);
        assert_eq!(serde_json::to_value(absent).unwrap(), json!({}));
        let null: Probe = serde_json::from_value(json!({"value":null})).unwrap();
        assert_eq!(null.value, Some(None));
        assert_eq!(serde_json::to_value(null).unwrap(), json!({"value":null}));
        for value in [
            json!(false),
            json!(0),
            json!(""),
            json!([]),
            json!({}),
            json!([null]),
        ] {
            let parsed: Probe = serde_json::from_value(json!({"value":value})).unwrap();
            assert_eq!(parsed.value, Some(Some(value.clone())));
            assert_eq!(
                serde_json::to_value(parsed).unwrap(),
                json!({"value":value})
            );
        }
    }
}
