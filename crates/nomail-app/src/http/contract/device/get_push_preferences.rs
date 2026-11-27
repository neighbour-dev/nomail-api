use super::endpoint;
use crate::device::PushPreferences as Preferences;
use crate::http::{Environment, HttpResult, dispatch::EndpointCall};
use serde::{Deserialize, Serialize};

endpoint! {
    GetPushPreferences {
        method: Get,
        path: "/api/device/push/preferences",
        auth: Bearer,
        operation_id: "getPushPreferences",
        tag: "Device",
        summary: "读取当前设备的推送设置",
        request: (),
        query: (),
        response: PushPreferences,
        body_kind: None,
        response_kind: Json,
        has_query: false,
        path_kind: None,
    }
}

impl GetPushPreferences {
    pub(in crate::http) async fn handle(
        call: EndpointCall<'_, impl Environment>,
    ) -> HttpResult<PushPreferences> {
        Ok(call
            .services
            .devices
            .push_preferences(call.actor()?)
            .await?
            .into())
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct PushPreferences {
    pub enabled: bool,
    pub low: bool,
    pub normal: bool,
    pub high: bool,
    pub urgent: bool,
}

impl From<Preferences> for PushPreferences {
    fn from(p: Preferences) -> Self {
        Self {
            enabled: p.enabled,
            low: p.low,
            normal: p.normal,
            high: p.high,
            urgent: p.urgent,
        }
    }
}

impl From<PushPreferences> for Preferences {
    fn from(p: PushPreferences) -> Self {
        Self {
            enabled: p.enabled,
            low: p.low,
            normal: p.normal,
            high: p.high,
            urgent: p.urgent,
        }
    }
}
