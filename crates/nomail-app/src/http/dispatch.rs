//! Shared HTTP execution before handing off to the matched endpoint.
use super::{
    ApiFailure, AuthRequirement, BodyKind, Endpoint, Environment, EnvironmentServices, HttpOutcome,
    HttpRequest, HttpResult, Operation, RequestHead, RouteMatch, bearer_token, check_body_length,
    endpoints, parse_json, parse_query, resolve,
};
use crate::{
    auth::{AuthContext, AuthRuntime},
    webhook::HeaderLookup,
};
use chrono::{DateTime, Utc};

pub(super) struct EndpointCall<'a, E: Environment> {
    pub(super) environment: &'a E,
    pub(super) services: &'a EnvironmentServices<E>,
    pub(super) matched: &'a RouteMatch,
    pub(super) head: &'a RequestHead,
    pub(super) body: &'a [u8],
    context: Option<AuthContext>,
}

impl<E: Environment> EndpointCall<'_, E> {
    pub(super) fn actor(&self) -> HttpResult<&AuthContext> {
        self.context
            .as_ref()
            .ok_or_else(|| ApiFailure::internal().into())
    }

    pub(super) fn input<T: Endpoint>(&self) -> HttpResult<T::Request> {
        Ok(parse_json::<T>(self.body, self.head.get("Content-Type"))?)
    }

    pub(super) fn query<T: Endpoint>(&self) -> HttpResult<T::Query> {
        Ok(parse_query::<T>(&self.head.query)?)
    }

    pub(super) fn now(&self) -> DateTime<Utc> {
        AuthRuntime::now(&self.services.auth.runtime)
    }
}

pub async fn execute(
    request: &mut impl HttpRequest,
    environment: &impl Environment,
) -> HttpResult<HttpOutcome> {
    let head = request.head();
    let matched = resolve(&head.method, &head.path)?;
    // Health must work without JWT/encryption/initial-account configuration.
    if matched.route.operation == Operation::Health {
        return endpoints::Health::respond(endpoints::Health::handle(environment, head).await?);
    }
    let services = environment.services()?;
    let context = if matched.route.auth == AuthRequirement::Bearer {
        Some(
            services
                .auth
                .authenticate(bearer_token(head.get("Authorization"))?)
                .await?,
        )
    } else {
        None
    };
    let body = if matched.route.body != BodyKind::None {
        let bytes = request.read_body().await?;
        check_body_length(bytes.len())?;
        bytes
    } else {
        Vec::new()
    };
    let head = request.head();
    matched
        .route
        .operation
        .handle(EndpointCall {
            environment,
            services: &services,
            matched: &matched,
            head,
            body: &body,
            context,
        })
        .await
}
