mod app;
mod config;
mod models;
mod routes;
mod services;
mod state;

use std::path::{Path, PathBuf};

use anyhow::Result;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::{config::AppConfig, state::AppState};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::from_filename(Path::new("../.env")).ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tlg_rag_bot=info,tower_http=info,axum=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 优先从环境变量 CONFIG_PATH 读取配置文件路径，默认使用 config.yaml
    let config_path = std::env::var("CONFIG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("config.yaml"));

    let config = AppConfig::load(&config_path)?;
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
