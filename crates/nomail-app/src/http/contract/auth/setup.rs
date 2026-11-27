use super::endpoint;
use crate::http::{Environment, HttpResult, dispatch::EndpointCall};
use serde::Deserialize;

endpoint! {
    Setup {
        method: Post,
        path: "/api/auth/setup",
        auth: Public,
        operation_id: "setup",
        tag: "Auth",
        summary: "首次初始化账号；无密钥保护，仅账号表为空时允许",
        request: SetupRequest,
        query: (),
        response: (),
        body_kind: Json,
        response_kind: Json,
        has_query: false,
        path_kind: None,
    }
}

impl Setup {
    pub(in crate::http) async fn handle(
        call: EndpointCall<'_, impl Environment>,
    ) -> HttpResult<()> {
        call.services
            .auth
            .initialize_account(call.input::<Self>()?.into())
            .await?;
        Ok(())
    }
}

#[derive(Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct SetupRequest {
    pub username: String,
    pub password: String,
}

impl From<SetupRequest> for crate::auth::InitializeAccount {
    fn from(request: SetupRequest) -> Self {
        Self {
            username: request.username,
            password: request.password,
        }
    }
}
