use super::endpoint;
use crate::http::{Environment, HttpResult, dispatch::EndpointCall};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

endpoint! {
    ListDevices {
        method: Get,
        path: "/api/device",
        auth: Bearer,
        operation_id: "listDevices",
        tag: "Device",
        summary: "设备列表",
        request: (),
        query: (),
        response: Vec<DeviceResponse>,
        body_kind: None,
        response_kind: Json,
        has_query: false,
        path_kind: None,
    }
}

impl ListDevices {
    pub(in crate::http) async fn handle(
        call: EndpointCall<'_, impl Environment>,
    ) -> HttpResult<Vec<DeviceResponse>> {
        Ok(call
            .services
            .devices
            .list(call.actor()?.user_id, call.now())
            .await?
            .into_iter()
            .map(Into::into)
            .collect())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct DeviceResponse {
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
    pub device_type: String,
    pub manufacturer: String,
    pub model: String,
    pub os_version: String,
    pub app_version: String,
    pub app_build: String,
    pub locale: String,
    pub time_zone: String,
    pub ip_address: String,
    pub user_agent: String,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub logged_in: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub logged_in_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<crate::device::DeviceInfo> for DeviceResponse {
    fn from(info: crate::device::DeviceInfo) -> Self {
        let d = info.device;
        Self {
            device_id: d.device_id,
            device_name: d.name.unwrap_or_default(),
            platform: d.platform,
            device_type: d.device_type,
            manufacturer: d.manufacturer.unwrap_or_default(),
            model: d.model.unwrap_or_default(),
            os_version: d.os_version.unwrap_or_default(),
            app_version: d.app_version.unwrap_or_default(),
            app_build: d.app_build.unwrap_or_default(),
            locale: d.locale.unwrap_or_default(),
            time_zone: d.time_zone.unwrap_or_default(),
            ip_address: d.ip_address.unwrap_or_default(),
            user_agent: d.user_agent.unwrap_or_default(),
            last_seen_at: d.last_seen_at,
            logged_in: info.logged_in,
            expires_at: info.session.as_ref().map(|session| session.expires_at),
            logged_in_at: info.session.as_ref().map(|session| session.logged_in_at),
            created_at: d.created_at,
            updated_at: d.updated_at,
        }
    }
}
