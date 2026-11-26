use actix_web::{App, HttpServer, web};
use nomail_app::push::{RelayConfig, TOKEN_SETTING};
use nomail_server::{AppState, app, config::Config, db, push, realtime::Hub};
use tracing_actix_web::TracingLogger;

#[actix_web::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    match dotenvy::dotenv() {
        Ok(_) => (),
        Err(dotenvy::Error::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(e.into()),
    }
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let config = Config::from_env()?;
    let relay = RelayConfig::from_token(std::env::var(TOKEN_SETTING).unwrap_or_default())?;
    let client = relay.map(push::HttpClient::new).transpose()?;
    let push_enabled = client.is_some();
    let port = config.port;
    let database = db::connect(&config).await?;
    // All HTTP workers must share one pool and one Hub.
    let state = web::Data::new(AppState {
        db: database.clone(),
        config,
        realtime: Hub::default(),
    });
    tracing::info!(port, "NoMail API listening");
    let server = HttpServer::new(move || {
        let mut app = App::new()
            .app_data(state.clone())
            .wrap(TracingLogger::default())
            .configure(app::configure);
        if push_enabled {
            app = app.app_data(push::Enabled);
        }
        app
    })
    .bind(("0.0.0.0", port))?
    .shutdown_timeout(10)
    .run();
    let push = client.map(|client| actix_web::rt::spawn(push::run(database.clone(), client)));
    let result = server.await;
    if let Some(push) = push {
        push.abort();
        let _ = push.await;
    }
    database.close().await?;
    result?;
    Ok(())
}
