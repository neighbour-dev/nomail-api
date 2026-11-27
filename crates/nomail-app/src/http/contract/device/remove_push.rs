use super::endpoint;
use crate::http::{Environment, HttpResult, dispatch::EndpointCall};

endpoint! {
    RemovePush {
        method: Delete,
        path: "/api/device/push/token",
        auth: Bearer,
        operation_id: "removePushToken",
        tag: "Device",
        summary: "删除当前登录设备的推送 Token",
        request: (),
        query: (),
        response: (),
        body_kind: None,
        response_kind: Json,
        has_query: false,
        path_kind: None,
    }
}

impl RemovePush {
    pub(in crate::http) async fn handle(
        call: EndpointCall<'_, impl Environment>,
    ) -> HttpResult<()> {
        call.services
            .devices
            .remove_push(call.actor()?, call.now())
            .await?;
        Ok(())
    }
}
