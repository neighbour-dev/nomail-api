//! Stripe-local extraction. Collections never silently drop invalid elements.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub(super) trait StripeValue: Sized {
    fn read(value: &Value) -> Option<Self>;
}

macro_rules! primitive {
    ($ty:ty, $method:ident) => {
        impl StripeValue for $ty {
            fn read(value: &Value) -> Option<Self> {
                value.$method()
            }
        }
    };
}
primitive!(bool, as_bool);
primitive!(i64, as_i64);

impl StripeValue for String {
    fn read(value: &Value) -> Option<Self> {
        value.as_str().map(str::to_owned)
    }
}
impl StripeValue for Value {
    fn read(value: &Value) -> Option<Self> {
        Some(value.clone())
    }
}
impl<T: StripeValue> StripeValue for Box<T> {
    fn read(value: &Value) -> Option<Self> {
        T::read(value).map(Box::new)
    }
}
impl<T: StripeValue> StripeValue for Option<T> {
    fn read(value: &Value) -> Option<Self> {
        if value.is_null() {
            Some(None)
        } else {
            T::read(value).map(Some)
        }
    }
}
impl<T: StripeValue> StripeValue for Vec<T> {
    fn read(value: &Value) -> Option<Self> {
        value.as_array()?.iter().map(T::read).collect()
    }
}
impl<T: StripeValue> StripeValue for BTreeMap<String, T> {
    fn read(value: &Value) -> Option<Self> {
        value
            .as_object()?
            .iter()
            .map(|(key, value)| Some((key.clone(), T::read(value)?)))
            .collect()
    }
}

/// Preserve a JSON number without an extra conversion through f64. Stripe's
/// decimal *strings* remain String; amounts in minor units remain i64.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct StripeNumber(
    #[cfg_attr(feature = "openapi", schemars(with = "f64"))] pub serde_json::Number,
);

impl StripeValue for StripeNumber {
    fn read(value: &Value) -> Option<Self> {
        value.as_number().cloned().map(Self)
    }
}

// Object discriminators are closed literal sets. Business status/type labels
// stay open strings, but a resource must not silently acquire another identity.
macro_rules! stripe_tag {
    ($name:ident { $($variant:ident = $wire:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
        #[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
        pub enum $name {
            $(#[serde(rename = $wire)] $variant,)+
        }
        impl $crate::webhook::platforms::stripe::support::StripeValue for $name {
            fn read(value: &serde_json::Value) -> Option<Self> {
                match value.as_str()? {
                    $($wire => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

macro_rules! stripe_model {
    ($name:ident $( [$guard:expr] )? { $($field:ident: $ty:ty = $key:literal),* $(,)? }) => {
        #[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
        #[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
        pub struct $name {
            $(#[serde(rename = $key, default, skip_serializing_if = "Option::is_none",
                deserialize_with = "crate::webhook::platforms::native::deserialize_present")]
            pub $field: crate::webhook::platforms::native::JsonField<$ty>,)*
        }
        impl $crate::webhook::platforms::stripe::support::StripeValue for $name {
            fn read(value: &serde_json::Value) -> Option<Self> {
                value.as_object()?;
                $(if !($guard)(value) { return None; })?
                Some(Self { $($field: crate::webhook::platforms::native::read_json_field(value, $key,
                    <$ty as $crate::webhook::platforms::stripe::support::StripeValue>::read),)* })
            }
        }
    };
}

// Custom deserialization shares the exact discriminator dispatch used by raw
// projections. Derived untagged deserialization would select the first optional
// resource struct, e.g. parsing a PaymentIntent as an Account.
macro_rules! stripe_union {
    ($name:ident { $($variant:ident($ty:ty) if $condition:expr),* $(,)? }) => {
        #[derive(Debug, Clone, PartialEq, serde::Serialize)]
        #[serde(untagged)]
        #[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
        pub enum $name {
            $($variant($ty),)*
            Unknown(std::collections::BTreeMap<String, serde_json::Value>),
        }
        impl $crate::webhook::platforms::stripe::support::StripeValue for $name {
            fn read(value: &serde_json::Value) -> Option<Self> {
                $(if ($condition)(value) {
                    return <$ty as $crate::webhook::platforms::stripe::support::StripeValue>::read(value)
                        .map(Self::$variant);
                })*
                <std::collections::BTreeMap<String, serde_json::Value> as
                    $crate::webhook::platforms::stripe::support::StripeValue>::read(value)
                    .map(Self::Unknown)
            }
        }
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = <serde_json::Value as serde::Deserialize>::deserialize(deserializer)?;
                <Self as $crate::webhook::platforms::stripe::support::StripeValue>::read(&value)
                    .ok_or_else(|| serde::de::Error::custom("invalid Stripe resource or expandable field"))
            }
        }
    };
}
