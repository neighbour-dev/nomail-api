use actix_web::{App, HttpServer, web};
use nomail_server::{AppState, app, auth, config::Config, db, realtime::Hub};
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
    let port = config.port;
    let database = db::connect(&config).await?;
    auth::ensure_initial_user(&database, &config).await?;
    // All HTTP workers must share one pool and one Hub.
    let state = web::Data::new(AppState {
        db: database.clone(),
        config,
        realtime: Hub::default(),
        health_limiter: app::HealthLimiter::default(),
    });
    tracing::info!(port, "NoMail API listening");
    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(TracingLogger::default())
            .configure(app::configure)
    })
    .bind(("0.0.0.0", port))?
    .shutdown_timeout(10)
    .run()
    .await?;
    database.close().await?;
    Ok(())
}
