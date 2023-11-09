use heed::types::Str;
use matrix_sdk::ruma::MilliSecondsSinceUnixEpoch;
use matrix_sdk::ruma::{OwnedEventId, OwnedRoomId, OwnedUserId};
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;

pub struct LuoxuBotContext {
    pub search: meilisearch_sdk::client::Client,
    pub db: heed::Database<Str, Str>,
    pub env: heed::Env,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct LuoxuConfig {
    pub matrix: LuoxuConfigMatrix,
    pub meilisearch: LuoxuConfigMeilisearch,
    pub state: LuoxuConfigState,
}

impl LuoxuConfig {
    pub fn from_string(config: String) -> anyhow::Result<LuoxuConfig> {
        Ok(toml::from_str(&config)?)
    }
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct LuoxuConfigMatrix {
    pub homeserver_url: String,
    pub username: String,
    pub password: Option<String>,
    pub device_name: String,
    pub indices: HashMap<String, String>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct LuoxuConfigMeilisearch {
    pub url: String,
    pub key: String,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct LuoxuConfigState {
    pub location: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(dead_code)]
pub struct LuoxuMessage {
    pub event_id: KeyEventId, // Primary
    pub body: String,
    pub external_url: Option<String>,
    pub user_id: OwnedUserId,
    pub user_display_name: Option<String>,
    pub timestamp: MilliSecondsSinceUnixEpoch,
    pub room_id: OwnedRoomId,
    pub ocr_body: Option<String>,
}

/// A Event ID for Meilisearch primary key.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyEventId(String);

impl From<OwnedEventId> for KeyEventId {
    fn from(event_id: OwnedEventId) -> Self {
        KeyEventId(event_id.as_str().strip_prefix('$').unwrap().to_string())
    }
}
