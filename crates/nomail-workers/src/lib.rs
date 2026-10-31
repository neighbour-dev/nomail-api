mod app;
mod auth;
mod config;
mod crypto;
mod db;
mod device;
mod error;
mod message;
mod realtime;
mod time;
mod webhook;
pub use realtime::Realtime;
use worker::*;

#[event(fetch)]
pub async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    match app::handle(req, env).await {
        Ok(response) => Ok(response),
        Err(error) => error.response(),
    }
}
