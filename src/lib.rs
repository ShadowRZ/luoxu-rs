use anyhow::Result;
use heed::EnvOpenOptions;
use heed::types::Str;
use matrix_sdk::ruma::MilliSecondsSinceUnixEpoch;
use matrix_sdk::ruma::{OwnedEventId, OwnedRoomId, OwnedUserId};
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;


#[derive(Clone)]
pub struct LuoxuBotContext {
    pub search: meilisearch_sdk::client::Client,
    pub store: HeedStore
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

#[derive(Clone)]
/// State store backed by Heed.
pub struct HeedStore {
    pub env: heed::Env,
    pub index_db: heed::Database<Str, Str>,
    pub name_db: heed::Database<Str, Str>,
}

impl HeedStore {
    pub fn new(location: &str) -> Result<Self> {
        let env = EnvOpenOptions::new().open(location)?;
        let mut wtxn = env.write_txn()?;
        let index_db = env.create_database(&mut wtxn, Some("index"))?;
        let name_db = env.create_database(&mut wtxn, Some("name"))?;
        wtxn.commit()?;
        Ok(HeedStore { env, index_db, name_db })
    }

    pub fn add_entry(&self, room_id: &str, index: &str, name: Option<&str>) -> Result<()> {
        let mut wtxn = self.env.write_txn()?;
        self.index_db.put(&mut wtxn, room_id, index)?;
        self.name_db.put(&mut wtxn, room_id, name.unwrap_or(room_id))?;
        wtxn.commit()?;
        Ok(())
    }

    pub fn move_entry(&self, old_room_id: &str, new_room_id: &str) -> Result<()> {
        let rtxn = self.env.read_txn()?;
        let index = self.index_db.get(&rtxn, old_room_id)?;
        let name = self.name_db.get(&rtxn, old_room_id)?;
        let mut wtxn = self.env.write_txn()?;
        self.index_db.put(&mut wtxn, new_room_id, index.unwrap())?;
        self.name_db.put(&mut wtxn, new_room_id, name.unwrap())?;
        wtxn.commit()?;
        Ok(())
    }

    pub fn update_entry(&self, room_id: &str, index: Option<&str>, name: Option<&str>) -> Result<()> {
        let mut wtxn = self.env.write_txn()?;
        if let Some(index) = index {
            self.index_db.put(&mut wtxn, room_id, index)?;
        }
        if let Some(name) = name {
            self.name_db.put(&mut wtxn, room_id, name)?;
        }
        wtxn.commit()?;
        Ok(())
    }

    pub fn get_index(&self, room_id: OwnedRoomId) -> Result<Option<String>> {
        let rtxn = self.env.read_txn()?;
        if let Ok(Some(index)) = self.index_db.get(&rtxn, room_id.as_str()) {
            Ok(Some(index.to_string()))
        } else {
            Ok(None)
        }
    }

    pub fn get_name(&self, room_id: OwnedRoomId) -> Result<Option<String>> {
        let rtxn = self.env.read_txn()?;
        if let Ok(Some(name)) = self.name_db.get(&rtxn, room_id.as_str()) {
            Ok(Some(name.to_string()))
        } else {
            Ok(None)
        }
    }
}