mod creature;
mod lifecycle;
mod store;
mod web;

use std::{env, net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use anyhow::Result;
use lifecycle::AppState;
use store::Store;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| "schrodingers_life=info".into()),
        )
        .init();

    let database =
        PathBuf::from(env::var("SCHRODINGER_DB").unwrap_or_else(|_| "schrodingers-life.db".into()));
    let grace_seconds = env::var("SCHRODINGER_GRACE_SECONDS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(30);
    let address: SocketAddr = env::var("SCHRODINGER_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".into())
        .parse()?;

    let state = Arc::new(AppState::new(
        Store::open(database)?,
        Duration::from_secs(grace_seconds),
    )?);
    let listener = tokio::net::TcpListener::bind(address).await?;

    info!(%address, grace_seconds, "observation apparatus online");
    axum::serve(listener, web::router(state)).await?;
    Ok(())
}
