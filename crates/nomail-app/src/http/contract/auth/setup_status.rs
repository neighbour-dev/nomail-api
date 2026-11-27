use super::endpoint;
use crate::http::{Environment, HttpResult, dispatch::EndpointCall};
use serde::{Deserialize, Serialize};

endpoint! {
    SetupStatus {
        method: Get,
        path: "/api/auth/setup",
        auth: Public,
        operation_id: "setupStatus",
        tag: "Auth",
        summary: "查询账号初始化状态",
        request: (),
        query: (),
        response: SetupStatusResponse,
        body_kind: None,
        response_kind: Json,
        has_query: false,
        path_kind: None,
    }
}

impl SetupStatus {
    pub(in crate::http) async fn handle(
        call: EndpointCall<'_, impl Environment>,
    ) -> HttpResult<SetupStatusResponse> {
        Ok(SetupStatusResponse {
            initialized: call.services.auth.is_initialized().await?,
        })
    }
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct SetupStatusResponse {
    pub initialized: bool,
}
