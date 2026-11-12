//! Typed projections of Apple's documented notification examples, not the
//! Webhook/WebhookEvent management API or App Store Server Notifications/JWS.
use crate::webhook::platforms::native::{JsonField, JsonFieldExt, read_json_field};
use crate::webhook::{WebhookError, support::required};
use serde::{Deserialize, Serialize};
use serde_json::Value;

fn string(value: &Value) -> Option<String> {
    value.as_str().map(str::to_owned)
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct AppStoreConnectWebhookPayload {
    pub data: AppStoreConnectNotification,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct AppStoreConnectNotification {
    #[serde(rename = "type")]
    pub event_type: String,
    /// The notification event ID, not the related app/build/feedback resource ID.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::webhook::platforms::native::deserialize_present"
    )]
    pub id: JsonField<String>,
    /// Present in some examples, absent in background-asset notifications.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::webhook::platforms::native::deserialize_present"
    )]
    pub version: JsonField<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::webhook::platforms::native::deserialize_present"
    )]
    pub attributes: JsonField<AppStoreConnectAttributes>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::webhook::platforms::native::deserialize_present"
    )]
    pub relationships: JsonField<AppStoreConnectRelationships>,
}

// Keep distinct attribute names. State strings are open-ended.
native_model!(AppStoreConnectAttributes {
    timestamp: String = "timestamp", new_value: String = "newValue", old_value: String = "oldValue",
    new_external_build_state: String = "newExternalBuildState", old_external_build_state: String = "oldExternalBuildState",
    new_state: String = "newState", old_state: String = "oldState", available: bool = "available",
    territories: Vec<String> = "territories", app_id: String = "appId",
});

native_model!(AppStoreConnectRelationships {
    instance: AppStoreConnectRelationship = "instance",
    marketplace_app: AppStoreConnectRelationship = "marketplaceApp",
});

/// Ordinary events wrap identifiers in `data`; background-asset examples put
/// id/type/links directly on the relationship. Preserve both documented shapes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct AppStoreConnectRelationship {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::webhook::platforms::native::deserialize_present"
    )]
    pub data: JsonField<AppStoreConnectResourceReference>,
    #[serde(flatten)]
    pub direct: AppStoreConnectResourceReference,
}

native_model!(AppStoreConnectResourceReference {
    resource_type: String = "type",
    id: String = "id",
    links: AppStoreConnectLinks = "links",
});

native_model!(AppStoreConnectLinks {
    self_url: String = "self",
    related: String = "related",
});

impl AppStoreConnectWebhookPayload {
    pub(super) fn from_payload(payload: &Value) -> Result<Self, WebhookError> {
        let data =
            payload
                .get("data")
                .filter(|v| v.is_object())
                .ok_or(WebhookError::InvalidPayload(
                    "missing App Store Connect data object",
                ))?;
        let event_type = string(&data["type"]).unwrap_or_default();
        required(event_type.clone(), "missing App Store Connect event type")?;
        Ok(Self {
            data: AppStoreConnectNotification {
                event_type,
                id: read_json_field(data, "id", string),
                version: read_json_field(data, "version", Value::as_u64),
                attributes: read_json_field(
                    data,
                    "attributes",
                    AppStoreConnectAttributes::from_value,
                ),
                relationships: read_json_field(
                    data,
                    "relationships",
                    AppStoreConnectRelationships::from_value,
                ),
            },
        })
    }
}

impl crate::webhook::platforms::native::NativeValue for AppStoreConnectWebhookPayload {
    fn read(value: &Value) -> Option<Self> {
        Self::from_payload(value).ok()
    }
}

impl AppStoreConnectAttributes {
    fn from_value(value: &Value) -> Option<Self> {
        <Self as crate::webhook::platforms::native::NativeValue>::read(value)
    }
}

impl AppStoreConnectRelationships {
    fn from_value(value: &Value) -> Option<Self> {
        <Self as crate::webhook::platforms::native::NativeValue>::read(value)
    }
}

impl AppStoreConnectRelationship {
    fn from_value(value: &Value) -> Option<Self> {
        value.as_object()?;
        Some(Self {
            data: read_json_field(value, "data", AppStoreConnectResourceReference::from_value),
            direct: AppStoreConnectResourceReference::from_value(value).unwrap_or_default(),
        })
    }

    pub fn reference(&self) -> &AppStoreConnectResourceReference {
        self.data.value().unwrap_or(&self.direct)
    }

    pub fn self_url(&self) -> Option<&str> {
        self.direct
            .links
            .value()
            .and_then(|l| l.self_url.value().map(String::as_str))
            .or_else(|| {
                self.data
                    .value()
                    .and_then(|r| r.links.value())
                    .and_then(|l| l.self_url.value().map(String::as_str))
            })
    }
}

impl AppStoreConnectResourceReference {
    fn from_value(value: &Value) -> Option<Self> {
        <Self as crate::webhook::platforms::native::NativeValue>::read(value)
    }
}

impl crate::webhook::platforms::native::NativeValue for AppStoreConnectRelationship {
    fn read(value: &Value) -> Option<Self> {
        Self::from_value(value)
    }
}
