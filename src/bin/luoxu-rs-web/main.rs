#![forbid(unsafe_code)]
pub mod routes;

use axum::{routing::get, Router};
use luoxu_rs::LuoxuConfig;
use std::net::SocketAddr;
use tokio::signal;

use crate::routes::{group_search, groups, index};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let config = LuoxuConfig::get_config()?;
    let context = config.get_context()?;

    // build our application with a single route
    let app = Router::new()
        .route("/", get(index))
        .route("/groups", get(groups))
        .route("/search/:index_name", get(group_search))
        .with_state(context.into());

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Listening on {}", addr);
    // run it with hyper on *:3000
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received, starting graceful shutdown");
}
