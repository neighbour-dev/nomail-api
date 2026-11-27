use super::endpoint;
use super::refresh::RefreshRequest;
use crate::http::{Environment, HttpResult, dispatch::EndpointCall};

endpoint! {
    Logout {
        method: Post,
        path: "/api/auth/logout",
        auth: Public,
        operation_id: "logout",
        tag: "Auth",
        summary: "退出登录",
        request: RefreshRequest,
        query: (),
        response: (),
        body_kind: Json,
        response_kind: Json,
        has_query: false,
        path_kind: None,
    }
}

impl Logout {
    pub(in crate::http) async fn handle(
        call: EndpointCall<'_, impl Environment>,
    ) -> HttpResult<()> {
        call.services
            .auth
            .logout(&call.input::<Self>()?.refresh_token)
            .await?;
        Ok(())
    }
}
