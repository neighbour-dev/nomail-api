use super::endpoint;
use super::login::TokenResponse;
use crate::http::{Environment, HttpResult, dispatch::EndpointCall};
use serde::Deserialize;

endpoint! {
    Refresh {
        method: Post,
        path: "/api/auth/refresh-token",
        auth: Public,
        operation_id: "refreshToken",
        tag: "Auth",
        summary: "轮换刷新令牌",
        request: RefreshRequest,
        query: (),
        response: TokenResponse,
        body_kind: Json,
        response_kind: Json,
        has_query: false,
        path_kind: None,
    }
}

impl Refresh {
    pub(in crate::http) async fn handle(
        call: EndpointCall<'_, impl Environment>,
    ) -> HttpResult<TokenResponse> {
        Ok(call
            .services
            .auth
            .refresh(
                &call.input::<Self>()?.refresh_token,
                call.head.metadata.clone(),
            )
            .await?
            .into())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct RefreshRequest {
    pub refresh_token: String,
}
