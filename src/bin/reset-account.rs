use clap::Parser;
use nomail_api::{auth, config::Config, db};

#[derive(Parser)]
#[command(about = "Reset the single-user account and revoke all login sessions")]
struct Args {
    /// Keep the current username when omitted.
    #[arg(long)]
    username: Option<String>,
    /// At least 8 characters, including ASCII letters and numbers; at most 72 bytes.
    #[arg(long)]
    password: String,
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    match dotenvy::dotenv() {
        Ok(_) => (),
        Err(dotenvy::Error::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(e.into()),
    }
    let config = Config::from_env()?;
    let database = db::connect(&config).await?;
    auth::reset_account(&database, args.username, args.password).await?;
    database.close().await?;
    println!("account reset successfully; all login sessions revoked");
    Ok(())
}
