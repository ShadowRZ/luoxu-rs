#![forbid(unsafe_code)]
pub mod routes;

use axum::{routing::get, Router};
use luoxu_rs::LuoxuConfig;
use std::net::SocketAddr;

use crate::routes::{group_search, groups};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let config = LuoxuConfig::get_config()?;
    let context = config.get_context()?;

    // build our application with a single route
    let app = Router::new()
        .route("/groups", get(groups))
        .route("/search/:index_name", get(group_search))
        .with_state(context.into());

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::info!("Listening on {}", addr);
    // run it with hyper on localhost:3000
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
