#![forbid(unsafe_code)]
use anyhow::Context;
use luoxu_rs::LuoxuConfig;
use matrix_sdk::Session;
use std::fs;

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

    let bot = LuoxuBot::new(config).await?;
    if let Some(session) = bot.login(login).await? {
        fs::write(SESSION_JSON_FILE, serde_json::to_string(&session)?)?;
    }

    // Run it
    bot.update_state().await?;
    bot.update_indices().await?;
    bot.run().await?;

    Ok(())
}
