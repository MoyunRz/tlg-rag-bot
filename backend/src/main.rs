mod app;
mod config;
mod models;
mod routes;
mod services;
mod state;

use anyhow::Result;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::{config::AppConfig, state::AppState};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tlg_rag_bot=info,tower_http=info,axum=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = AppConfig::from_env()?;
    let bind_addr = config.bind_addr();
    let state = AppState::new(config)?;
    let app = app::build_router(state.clone());
    let listener = TcpListener::bind(&bind_addr).await?;
    let server = axum::serve(listener, app).with_graceful_shutdown(shutdown_signal());

    tracing::info!(
        address = %bind_addr,
        telegram_enabled = state.config.telegram_enabled,
        "backend listening"
    );

    if state.config.telegram_enabled {
        let telegram = tokio::spawn(routes::telegram::run_long_polling(state));

        tokio::select! {
            result = server => result?,
            result = telegram => result??,
        }
    } else {
        tracing::info!("telegram long polling disabled");
        server.await?;
    }

    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C handler");

    tracing::info!("shutdown signal received");
}
