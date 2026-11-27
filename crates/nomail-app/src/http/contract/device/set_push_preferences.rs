use super::endpoint;
use super::get_push_preferences::PushPreferences;
use crate::device::DeviceStore;
use crate::http::{Environment, HttpResult, dispatch::EndpointCall};

endpoint! {
    SetPushPreferences {
        method: Put,
        path: "/api/device/push/preferences",
        auth: Bearer,
        operation_id: "setPushPreferences",
        tag: "Device",
        summary: "更新当前设备的推送总开关和等级开关",
        request: PushPreferences,
        query: (),
        response: (),
        body_kind: Json,
        response_kind: Json,
        has_query: false,
        path_kind: None,
    }
}

impl SetPushPreferences {
    pub(in crate::http) async fn handle(
        call: EndpointCall<'_, impl Environment>,
    ) -> HttpResult<()> {
        call.services
            .devices
            .store
            .set_push_preferences(call.actor()?, call.input::<Self>()?.into(), call.now())
            .await?;
        Ok(())
    }
}
