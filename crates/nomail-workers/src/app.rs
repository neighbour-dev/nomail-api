//! Workers transport binds Core operations, never paths/methods/DTO definitions.
use crate::{
    auth,
    config::Config,
    db::Store,
    device,
    error::{ApiError, Result, success},
    message, webhook,
};
use futures_util::StreamExt;
use nomail_core::api::{
    self, AuthRequirement, BodyKind, HealthResponse, Operation, endpoints as e,
};
use nomail_core::auth::AuthContext;
use worker::{Env, Request, Response};

async fn body(req: &mut Request) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut stream = req.stream()?;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        api::check_body_length(bytes.len().saturating_add(chunk.len()))?;
        bytes.extend(chunk);
    }
    Ok(bytes)
}
fn actor(context: &Option<AuthContext>) -> Result<i64> {
    context.as_ref().map(|ctx| ctx.user_id).ok_or_else(|| {
        worker::Error::RustError("missing authenticated route context".into()).into()
    })
}

pub async fn handle(mut req: Request, env: Env) -> Result<Response> {
    let matched = api::resolve(req.method().as_ref(), &req.path())?;
    let db = Store(env.d1("DB")?);
    // Health doesn't need auth/encryption secrets, but the binding remains an
    // explicit exhaustive arm in the dispatcher below.
    let config = if matched.route.operation == Operation::Health {
        None
    } else {
        Some(Config::load(&env)?)
    };
    let context = if matched.route.auth == AuthRequirement::Bearer {
        Some(
            auth::authenticate(
                &req,
                &db,
                config
                    .as_ref()
                    .ok_or_else(|| ApiError::from(api::ApiFailure::internal()))?,
            )
            .await?,
        )
    } else {
        None
    };
    let bytes = if matched.route.body != BodyKind::None {
        body(&mut req).await?
    } else {
        Vec::new()
    };
    let content_type = req.headers().get("Content-Type")?;
    let query = req.url()?.query().unwrap_or("").to_owned();
    macro_rules! config {
        () => {
            config
                .as_ref()
                .ok_or_else(|| ApiError::from(api::ApiFailure::internal()))?
        };
    }
    macro_rules! json {
        ($endpoint:ty) => {
            api::parse_json::<$endpoint>(&bytes, content_type.as_deref())?
        };
    }
    macro_rules! id {
        () => {
            matched.parameter()?
        };
    }
    match matched.route.operation {
        Operation::Health => success::<e::Health>(HealthResponse::database(
            db.rows("SELECT 1", vec![]).await.is_ok(),
        )),
        Operation::Login => auth::login(&req, json!(e::Login), &db, config!(), &env).await,
        Operation::Refresh => auth::refresh(&req, json!(e::Refresh), &db, config!()).await,
        Operation::Logout => auth::logout(json!(e::Logout), &db, config!()).await,
        Operation::GetAccount => auth::get_account(actor(&context)?, &db, config!()).await,
        Operation::UpdateAccount => {
            auth::update_account(json!(e::UpdateAccount), actor(&context)?, &db, config!()).await
        }
        Operation::ListDevices => device::list(actor(&context)?, &db, config!()).await,
        Operation::KickDevice => device::kick(id!(), actor(&context)?, &db, config!()).await,
        Operation::DeleteDevice => device::delete(id!(), actor(&context)?, &db, config!()).await,
        Operation::ListConfigs => {
            webhook::configs::list(
                api::parse_query::<e::ListConfigs>(&query)?,
                actor(&context)?,
                &db,
                config!(),
            )
            .await
        }
        Operation::CreateConfig => {
            webhook::configs::create(json!(e::CreateConfig), actor(&context)?, &db, config!()).await
        }
        Operation::GetConfig => {
            webhook::configs::get(id!(), actor(&context)?, &db, config!()).await
        }
        Operation::UpdateConfig => {
            webhook::configs::update(
                id!(),
                json!(e::UpdateConfig),
                actor(&context)?,
                &db,
                config!(),
            )
            .await
        }
        Operation::DeleteConfig => {
            webhook::configs::delete(id!(), actor(&context)?, &db, config!()).await
        }
        Operation::EnableConfig => {
            webhook::configs::enabled(
                id!(),
                json!(e::EnableConfig),
                actor(&context)?,
                &db,
                config!(),
            )
            .await
        }
        Operation::ReceiveWebhook => {
            webhook::receive(&req, &bytes, id!(), &db, config!(), &env).await
        }
        Operation::ListMessages => {
            message::list(
                api::parse_query::<e::ListMessages>(&query)?,
                actor(&context)?,
                &db,
            )
            .await
        }
        Operation::UnreadCount => message::unread_count(actor(&context)?, &db).await,
        Operation::StreamMessages => {
            let ctx = context.ok_or_else(|| ApiError::from(api::ApiFailure::internal()))?;
            Ok(crate::realtime::subscribe(&env, ctx.user_id, ctx.expires_at.timestamp()).await?)
        }
        Operation::BatchMessages => {
            message::batch(json!(e::BatchMessages), actor(&context)?, &db).await
        }
        Operation::GetMessage => message::get(id!(), actor(&context)?, &db).await,
        Operation::UpdateMessage => {
            message::update(id!(), json!(e::UpdateMessage), actor(&context)?, &db).await
        }
        Operation::DeleteMessage => message::delete(id!(), actor(&context)?, &db).await,
        Operation::RawMessage => message::raw(id!(), actor(&context)?, &db).await,
    }
}
