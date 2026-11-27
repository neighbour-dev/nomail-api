use super::endpoint;
use crate::http::{Environment, HttpResult, dispatch::EndpointCall};

endpoint! {
    DeleteDevice {
        method: Delete,
        path: "/api/device/{deviceId}",
        auth: Bearer,
        operation_id: "deleteDevice",
        tag: "Device",
        summary: "删除设备",
        request: (),
        query: (),
        response: (),
        body_kind: None,
        response_kind: Json,
        has_query: false,
        path_kind: DeviceId,
    }
}

impl DeleteDevice {
    pub(in crate::http) async fn handle(
        call: EndpointCall<'_, impl Environment>,
    ) -> HttpResult<()> {
        call.services
            .devices
            .delete(call.actor()?.user_id, call.matched.parameter()?)
            .await?;
        Ok(())
    }
}
