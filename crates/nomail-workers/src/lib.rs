mod app;
mod auth;
mod config;
mod crypto;
mod db;
mod device;
mod error;
mod message;
mod push;
mod realtime;
mod runtime;
mod services;
mod time;
mod webhook;
pub use realtime::Realtime;
use worker::*;

#[event(fetch)]
pub async fn fetch(req: Request, env: Env, ctx: Context) -> Result<Response> {
    match app::handle(req, env, ctx).await {
        Ok(response) => Ok(response),
        Err(error) => error.response(),
    }
}

#[event(scheduled)]
pub async fn scheduled(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    match push::config(&env) {
        Ok(Some(config)) => push::run(env, config, 16).await,
        Ok(None) => (),
        Err(message) => console_error!("{message}"),
    }
}
