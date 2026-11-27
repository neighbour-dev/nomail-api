use super::endpoint;
use crate::http::{Environment, HttpResult, dispatch::EndpointCall};

endpoint! {
    KickDevice {
        method: Delete,
        path: "/api/device/{deviceId}/session",
        auth: Bearer,
        operation_id: "kickDevice",
        tag: "Device",
        summary: "撤销设备会话",
        request: (),
        query: (),
        response: (),
        body_kind: None,
        response_kind: Json,
        has_query: false,
        path_kind: DeviceId,
    }
}

impl KickDevice {
    pub(in crate::http) async fn handle(
        call: EndpointCall<'_, impl Environment>,
    ) -> HttpResult<()> {
        call.services
            .devices
            .revoke_session(call.actor()?.user_id, call.matched.device_id()?, call.now())
            .await?;
        Ok(())
    }
}
