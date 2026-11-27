use super::endpoint;
use crate::http::{Environment, HttpResult, dispatch::EndpointCall};
use serde::{Deserialize, Serialize};

endpoint! {
    GetAccount {
        method: Get,
        path: "/api/auth/account",
        auth: Bearer,
        operation_id: "getAccount",
        tag: "Auth",
        summary: "获取账号",
        request: (),
        query: (),
        response: AccountResponse,
        body_kind: None,
        response_kind: Json,
        has_query: false,
        path_kind: None,
    }
}

impl GetAccount {
    pub(in crate::http) async fn handle(
        call: EndpointCall<'_, impl Environment>,
    ) -> HttpResult<AccountResponse> {
        let account = call.services.auth.account(call.actor()?.user_id).await?;
        Ok(account.into())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct AccountResponse {
    pub id: i64,
    pub username: String,
}

impl From<crate::auth::AccountInfo> for AccountResponse {
    fn from(account: crate::auth::AccountInfo) -> Self {
        Self {
            id: account.id,
            username: account.username,
        }
    }
}
