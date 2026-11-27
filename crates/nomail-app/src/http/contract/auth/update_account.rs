use super::endpoint;
use super::get_account::AccountResponse;
use crate::http::{Environment, HttpResult, dispatch::EndpointCall};
use serde::Deserialize;

endpoint! {
    UpdateAccount {
        method: Patch,
        path: "/api/auth/account",
        auth: Bearer,
        operation_id: "updateAccount",
        tag: "Auth",
        summary: "修改账号",
        request: AccountUpdateRequest,
        query: (),
        response: AccountResponse,
        body_kind: Json,
        response_kind: Json,
        has_query: false,
        path_kind: None,
    }
}

impl UpdateAccount {
    pub(in crate::http) async fn handle(
        call: EndpointCall<'_, impl Environment>,
    ) -> HttpResult<AccountResponse> {
        Ok(call
            .services
            .auth
            .update_account(call.actor()?.user_id, call.input::<Self>()?.into())
            .await?
            .into())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct AccountUpdateRequest {
    pub current_password: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub new_password: String,
}

impl From<AccountUpdateRequest> for crate::auth::UpdateAccount {
    fn from(request: AccountUpdateRequest) -> Self {
        Self {
            current_password: request.current_password,
            username: request.username,
            new_password: request.new_password,
        }
    }
}
