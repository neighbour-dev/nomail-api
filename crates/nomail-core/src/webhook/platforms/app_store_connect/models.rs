//! Typed projections of Apple's documented notification examples, not the
//! Webhook/WebhookEvent management API or App Store Server Notifications/JWS.
use crate::webhook::{WebhookError, support::required};
use serde::{Deserialize, Serialize};
use serde_json::Value;

fn string(value: &Value) -> Option<String> {
    value.as_str().map(str::to_owned)
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct AppStoreConnectEvent {
    pub data: AppStoreConnectNotification,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct AppStoreConnectNotification {
    #[serde(rename = "type")]
    pub event_type: String,
    /// The notification event ID, not the related app/build/feedback resource ID.
    pub id: Option<String>,
    /// Present in some examples, absent in background-asset notifications.
    pub version: Option<u64>,
    pub attributes: Option<AppStoreConnectAttributes>,
    pub relationships: Option<AppStoreConnectRelationships>,
}

/// Keep the distinct attribute names from each documented event family. State
/// strings are open-ended; a new Apple state must not reject the notification.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct AppStoreConnectAttributes {
    pub timestamp: Option<String>,
    pub new_value: Option<String>,
    pub old_value: Option<String>,
    pub new_external_build_state: Option<String>,
    pub old_external_build_state: Option<String>,
    pub new_state: Option<String>,
    pub old_state: Option<String>,
    pub available: Option<bool>,
    pub territories: Vec<String>,
    pub app_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct AppStoreConnectRelationships {
    pub instance: Option<AppStoreConnectRelationship>,
    pub marketplace_app: Option<AppStoreConnectRelationship>,
}

/// Ordinary events wrap identifiers in `data`; background-asset examples put
/// id/type/links directly on the relationship. Preserve both documented shapes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct AppStoreConnectRelationship {
    pub data: Option<AppStoreConnectResourceReference>,
    #[serde(flatten)]
    pub direct: AppStoreConnectResourceReference,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct AppStoreConnectResourceReference {
    #[serde(rename = "type")]
    pub resource_type: Option<String>,
    pub id: Option<String>,
    pub links: Option<AppStoreConnectLinks>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct AppStoreConnectLinks {
    #[serde(rename = "self")]
    pub self_url: Option<String>,
    pub related: Option<String>,
}

impl AppStoreConnectEvent {
    pub(super) fn from_payload(payload: &Value) -> Result<Self, WebhookError> {
        let data =
            payload
                .get("data")
                .filter(|v| v.is_object())
                .ok_or(WebhookError::InvalidPayload(
                    "missing App Store Connect data object",
                ))?;
        let event_type = required(
            string(&data["type"]).unwrap_or_default(),
            "missing App Store Connect event type",
        )?;
        Ok(Self {
            data: AppStoreConnectNotification {
                event_type,
                id: string(&data["id"]),
                version: data["version"].as_u64(),
                attributes: AppStoreConnectAttributes::from_value(&data["attributes"]),
                relationships: AppStoreConnectRelationships::from_value(&data["relationships"]),
            },
        })
    }
}

impl AppStoreConnectAttributes {
    fn from_value(value: &Value) -> Option<Self> {
        value.as_object()?;
        Some(Self {
            timestamp: string(&value["timestamp"]),
            new_value: string(&value["newValue"]),
            old_value: string(&value["oldValue"]),
            new_external_build_state: string(&value["newExternalBuildState"]),
            old_external_build_state: string(&value["oldExternalBuildState"]),
            new_state: string(&value["newState"]),
            old_state: string(&value["oldState"]),
            available: value["available"].as_bool(),
            territories: value["territories"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(string)
                .collect(),
            app_id: string(&value["appId"]),
        })
    }
}

impl AppStoreConnectRelationships {
    fn from_value(value: &Value) -> Option<Self> {
        value.as_object()?;
        Some(Self {
            instance: AppStoreConnectRelationship::from_value(&value["instance"]),
            marketplace_app: AppStoreConnectRelationship::from_value(&value["marketplaceApp"]),
        })
    }
}

impl AppStoreConnectRelationship {
    fn from_value(value: &Value) -> Option<Self> {
        value.as_object()?;
        Some(Self {
            data: AppStoreConnectResourceReference::from_value(&value["data"]),
            direct: AppStoreConnectResourceReference::from_value(value).unwrap_or_default(),
        })
    }

    pub fn reference(&self) -> &AppStoreConnectResourceReference {
        self.data.as_ref().unwrap_or(&self.direct)
    }

    pub fn self_url(&self) -> Option<&str> {
        self.direct
            .links
            .as_ref()
            .and_then(|l| l.self_url.as_deref())
            .or_else(|| {
                self.data
                    .as_ref()
                    .and_then(|r| r.links.as_ref())
                    .and_then(|l| l.self_url.as_deref())
            })
    }
}

impl AppStoreConnectResourceReference {
    fn from_value(value: &Value) -> Option<Self> {
        value.as_object()?;
        Some(Self {
            resource_type: string(&value["type"]),
            id: string(&value["id"]),
            links: AppStoreConnectLinks::from_value(&value["links"]),
        })
    }
}

impl AppStoreConnectLinks {
    fn from_value(value: &Value) -> Option<Self> {
        value.as_object()?;
        Some(Self {
            self_url: string(&value["self"]),
            related: string(&value["related"]),
        })
    }
}
