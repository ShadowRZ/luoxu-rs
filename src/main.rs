#![forbid(unsafe_code)]
use anyhow::Context;
use luoxu_rs::LuoxuConfig;
use matrix_sdk::Session;
use std::fs;
use tokio::{signal, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::bot::{LoginType, LuoxuBot};

mod bot;
mod callbacks;

static SESSION_JSON_FILE: &str = "credentials.json";

fn get_session() -> anyhow::Result<Session> {
    Ok(serde_json::from_str::<Session>(&fs::read_to_string(
        SESSION_JSON_FILE,
    )?)?)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config = LuoxuConfig::get_config().context("Failed to read config file")?;

    let session = get_session();

    let login = match &config.matrix.password {
        None => LoginType::Session(
            session.context("Saved credentials not found without specifing password")?,
        ),
        Some(password) => match session {
            Ok(session) => LoginType::Session(session),
            Err(_) => LoginType::Password(password.to_string()),
        },
    };

    let cts = CancellationToken::new();
    let bot_cts = cts.clone();

    let bot = LuoxuBot::new(config).await?;
    if let Some(session) = bot.login(login).await? {
        fs::write(SESSION_JSON_FILE, serde_json::to_string(&session)?)?;
    }

    {
        tokio::spawn(async move {
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
                _ = ctrl_c => {
                    cts.cancel();
                },
                _ = terminate => {
                    cts.cancel();
                },
            }
        });
    }
    // Run it
    bot.update_state().await?;
    bot.update_indices().await?;
    let task: JoinHandle<anyhow::Result<()>> = tokio::spawn(async move{
        tokio::select! {
            _ = bot_cts.cancelled() => {
                tracing::info!("Shutdown signal received, starting graceful shutdown");
                Ok(())
            }
            _ = bot.run() => {
                Ok(())
            }
        }
    });

    task.await?
}
