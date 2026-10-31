//! The only binding of API operations to community use cases.
use super::{
    ApiFailure, AuthRequirement, BodyKind, Environment, HealthCheck, HealthResponse, HttpOutcome,
    HttpReply, HttpRequest, HttpResult, Operation, RealtimeEffect, bearer_token, check_body_length,
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
        return match environment.health(head).await? {
            HealthCheck::Database(healthy) => json::<e::Health>(HealthResponse::database(healthy)),
            HealthCheck::Throttled { retry_after } => {
                let body =
                    serde_json::to_vec(&ApiFailure::new(429, "Too Many Requests").response())
                        .map_err(|_| ApiFailure::internal())?;
                let mut outcome = HttpOutcome::response(429, "application/json", body);
                if let HttpReply::Response(response) = &mut outcome.reply {
                    response
                        .headers
                        .push(("Retry-After".into(), retry_after.to_string()));
                }
                Ok(outcome)
            }
        };
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
    macro_rules! id {
        () => {
            uuid::Uuid::parse_str(matched.parameter()?)
                .map_err(|_| ApiFailure::bad("invalid message id"))?
        };
    }
    match matched.route.operation {
        Operation::Health => unreachable!("health is handled before service configuration"),
        Operation::Login => json::<e::Login>(
            services
                .auth
                .login(input!(e::Login), head.metadata.clone())
                .await?,
        ),
        Operation::Refresh => json::<e::Refresh>(
            services
                .auth
                .refresh(input!(e::Refresh), head.metadata.clone())
                .await?,
        ),
        Operation::Logout => {
            services.auth.logout(input!(e::Logout)).await?;
            json::<e::Logout>(())
        }
        Operation::GetAccount => json::<e::GetAccount>(services.auth.account(user!()).await?),
        Operation::UpdateAccount => json::<e::UpdateAccount>(
            services
                .auth
                .update_account(user!(), input!(e::UpdateAccount))
                .await?,
        ),
        Operation::ListDevices => json::<e::ListDevices>(services.auth.devices(user!()).await?),
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
                .list(user!(), parse_query::<e::ListConfigs>(&head.query)?)
                .await?
                .into(),
        ),
        Operation::CreateConfig => json::<e::CreateConfig>(
            services
                .configs
                .create(user!(), input!(e::CreateConfig))
                .await?
                .into(),
        ),
        Operation::GetConfig => json::<e::GetConfig>(
            services
                .configs
                .get(user!(), matched.parameter()?)
                .await?
                .into(),
        ),
        Operation::UpdateConfig => json::<e::UpdateConfig>(
            services
                .configs
                .update(user!(), matched.parameter()?, input!(e::UpdateConfig))
                .await?
                .into(),
        ),
        Operation::DeleteConfig => {
            services
                .configs
                .delete(user!(), matched.parameter()?)
                .await?;
            json::<e::DeleteConfig>(())
        }
        Operation::EnableConfig => json::<e::EnableConfig>(
            services
                .configs
                .enabled(
                    user!(),
                    matched.parameter()?,
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
                .list(user!(), parse_query::<e::ListMessages>(&head.query)?)
                .await?,
        ),
        Operation::UnreadCount => {
            json::<e::UnreadCount>(services.messages.unread_count(user!()).await?)
        }
        Operation::StreamMessages => {
            let context = actor(&context)?;
            Ok(HttpOutcome::subscribe(context.user_id, context.expires_at))
        }
        Operation::BatchMessages => json::<e::BatchMessages>(
            services
                .messages
                .batch(
                    user!(),
                    input!(e::BatchMessages),
                    AuthRuntime::now(&services.auth.runtime),
                )
                .await?,
        ),
        Operation::GetMessage => {
            json::<e::GetMessage>(services.messages.get(user!(), id!()).await?)
        }
        Operation::UpdateMessage => json::<e::UpdateMessage>(
            services
                .messages
                .update(
                    user!(),
                    id!(),
                    input!(e::UpdateMessage),
                    AuthRuntime::now(&services.auth.runtime),
                )
                .await?,
        ),
        Operation::DeleteMessage => {
            services
                .messages
                .delete(user!(), id!(), AuthRuntime::now(&services.auth.runtime))
                .await?;
            json::<e::DeleteMessage>(())
        }
        Operation::RawMessage => {
            let raw = services.messages.raw(user!(), id!()).await?;
            Ok(HttpOutcome::response(
                200,
                raw.content_type,
                raw.text.into_bytes(),
            ))
        }
    }
}
