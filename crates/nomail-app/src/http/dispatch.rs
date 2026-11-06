//! The only binding of API operations to community use cases.
use super::{
    ApiFailure, AuthRequirement, BodyKind, Environment, HealthCheck, HealthResponse, HttpOutcome,
    HttpRequest, HttpResult, Operation, RealtimeEffect, bearer_token, check_body_length,
    endpoints as e, parse_json, parse_query, resolve, transport::json, webhook::WebhookReply,
};
use crate::{
    auth::{AuthContext, AuthRuntime},
    webhook::{HeaderLookup, WebhookRequest},
};

fn actor(context: &Option<AuthContext>) -> HttpResult<&AuthContext> {
    context
        .as_ref()
        .ok_or_else(|| ApiFailure::internal().into())
}

pub async fn execute(
    request: &mut impl HttpRequest,
    environment: &impl Environment,
) -> HttpResult<HttpOutcome> {
    let head = request.head();
    let matched = resolve(&head.method, &head.path)?;
    // Health must work without JWT/encryption/initial-account configuration.
    if matched.route.operation == Operation::Health {
        let HealthCheck::Database(healthy) = environment.health(head).await?;
        return json::<e::Health>(HealthResponse::database(healthy));
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
    macro_rules! input {
        ($endpoint:ty) => {
            parse_json::<$endpoint>(&body, head.get("Content-Type"))?
        };
    }
    macro_rules! user {
        () => {
            actor(&context)?.user_id
        };
    }
    match matched.route.operation {
        Operation::Health => unreachable!("health is handled before service configuration"),
        Operation::Login => json::<e::Login>(
            services
                .auth
                .login(input!(e::Login).into(), head.metadata.clone())
                .await?
                .into(),
        ),
        Operation::Refresh => json::<e::Refresh>(
            services
                .auth
                .refresh(&input!(e::Refresh).refresh_token, head.metadata.clone())
                .await?
                .into(),
        ),
        Operation::Logout => {
            services
                .auth
                .logout(&input!(e::Logout).refresh_token)
                .await?;
            json::<e::Logout>(())
        }
        Operation::GetAccount => {
            json::<e::GetAccount>(services.auth.account(user!()).await?.into())
        }
        Operation::UpdateAccount => json::<e::UpdateAccount>(
            services
                .auth
                .update_account(user!(), input!(e::UpdateAccount).into())
                .await?
                .into(),
        ),
        Operation::ListDevices => json::<e::ListDevices>(
            services
                .auth
                .devices(user!())
                .await?
                .into_iter()
                .map(Into::into)
                .collect(),
        ),
        Operation::KickDevice => {
            services
                .auth
                .kick_device(user!(), matched.parameter()?)
                .await?;
            json::<e::KickDevice>(())
        }
        Operation::DeleteDevice => {
            services
                .auth
                .delete_device(user!(), matched.parameter()?)
                .await?;
            json::<e::DeleteDevice>(())
        }
        Operation::ListConfigs => json::<e::ListConfigs>(
            services
                .configs
                .list(
                    user!(),
                    parse_query::<e::ListConfigs>(&head.query)?.try_into()?,
                )
                .await?
                .into(),
        ),
        Operation::CreateConfig => json::<e::CreateConfig>(
            services
                .configs
                .create(user!(), input!(e::CreateConfig).try_into()?)
                .await?
                .into(),
        ),
        Operation::GetConfig => json::<e::GetConfig>(
            services
                .configs
                .get(user!(), matched.config_id()?)
                .await?
                .into(),
        ),
        Operation::UpdateConfig => json::<e::UpdateConfig>(
            services
                .configs
                .update(
                    user!(),
                    matched.config_id()?,
                    input!(e::UpdateConfig).try_into()?,
                )
                .await?
                .into(),
        ),
        Operation::DeleteConfig => {
            services
                .configs
                .delete(user!(), matched.config_id()?)
                .await?;
            json::<e::DeleteConfig>(())
        }
        Operation::EnableConfig => json::<e::EnableConfig>(
            services
                .configs
                .enabled(
                    user!(),
                    matched.config_id()?,
                    input!(e::EnableConfig).enabled,
                )
                .await?
                .into(),
        ),
        Operation::ReceiveWebhook => {
            let outcome = services
                .ingestion
                .receive(
                    matched.parameter()?,
                    &WebhookRequest {
                        headers: head,
                        body: &body,
                    },
                )
                .await?;
            match WebhookReply::from(outcome) {
                WebhookReply::Control(response) => Ok(HttpOutcome::response(
                    response.status,
                    response.content_type,
                    response.body,
                )),
                WebhookReply::Acknowledgement { response, created } => {
                    let mut outcome = json::<e::ReceiveWebhook>(response)?;
                    if let Some((user_id, event)) = created {
                        outcome.events.push(RealtimeEffect { user_id, event });
                    }
                    Ok(outcome)
                }
            }
        }
        Operation::ListMessages => json::<e::ListMessages>(
            services
                .messages
                .list(
                    user!(),
                    parse_query::<e::ListMessages>(&head.query)?.try_into()?,
                )
                .await?
                .into(),
        ),
        Operation::CursorMessages => json::<e::CursorMessages>(
            services
                .messages
                .cursor(
                    user!(),
                    parse_query::<e::CursorMessages>(&head.query)?.try_into()?,
                )
                .await?
                .into(),
        ),
        Operation::UnreadCount => json::<e::UnreadCount>(super::CountResponse {
            count: services.messages.unread_count(user!()).await?,
        }),
        Operation::StreamMessages => {
            let context = actor(&context)?;
            Ok(HttpOutcome::subscribe(context.user_id, context.expires_at))
        }
        Operation::BatchMessages => json::<e::BatchMessages>(super::AffectedResponse {
            affected: services
                .messages
                .batch(
                    user!(),
                    input!(e::BatchMessages).try_into()?,
                    AuthRuntime::now(&services.auth.runtime),
                )
                .await?,
        }),
        Operation::GetMessage => json::<e::GetMessage>(
            services
                .messages
                .get(user!(), matched.message_id()?)
                .await?
                .into(),
        ),
        Operation::UpdateMessage => json::<e::UpdateMessage>(
            services
                .messages
                .update(
                    user!(),
                    matched.message_id()?,
                    input!(e::UpdateMessage).try_into()?,
                    AuthRuntime::now(&services.auth.runtime),
                )
                .await?
                .into(),
        ),
        Operation::DeleteMessage => {
            services
                .messages
                .delete(
                    user!(),
                    matched.message_id()?,
                    AuthRuntime::now(&services.auth.runtime),
                )
                .await?;
            json::<e::DeleteMessage>(())
        }
        Operation::RawMessage => {
            let raw = services
                .messages
                .raw(user!(), matched.message_id()?)
                .await?;
            Ok(HttpOutcome::response(
                200,
                raw.content_type,
                raw.text.into_bytes(),
            ))
        }
    }
}
