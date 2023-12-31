use anyhow::Result;
use heed::types::Str;
use heed::EnvOpenOptions;
use matrix_sdk::reqwest::Url;
use matrix_sdk::ruma::MilliSecondsSinceUnixEpoch;
use matrix_sdk::ruma::{MxcUri, OwnedEventId, OwnedRoomId, OwnedUserId};
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;

static CONFIG_FILE: &str = "luoxu-rs.toml";

#[derive(Clone)]
pub struct LuoxuBotContext {
    pub search: meilisearch_sdk::client::Client,
    pub store: HeedStore,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct LuoxuConfig {
    pub matrix: LuoxuConfigMatrix,
    pub meilisearch: LuoxuConfigMeilisearch,
    pub state: LuoxuConfigState,
}

impl LuoxuConfig {
    pub fn from_string(config: String) -> anyhow::Result<Self> {
        Ok(toml::from_str(&config)?)
    }

    pub fn get_config() -> anyhow::Result<Self> {
        use std::fs;
        Self::from_string(fs::read_to_string(CONFIG_FILE)?)
    }

    pub fn get_context(&self) -> anyhow::Result<LuoxuBotContext> {
        use std::fs;
        let config = self;
        // Create state dirs.
        let _ = fs::create_dir_all(&config.state.location);

        let store = HeedStore::new(&config.state.location)?;
        let context = LuoxuBotContext {
            search: meilisearch_sdk::client::Client::new(
                &config.meilisearch.url,
                Some(&config.meilisearch.key),
            ),
            store,
        };
        Ok(context)
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
    pub user_avatar: Option<LuoxuAvatar>,
    pub timestamp: MilliSecondsSinceUnixEpoch,
    pub room_id: OwnedRoomId,
    pub ocr_body: Option<String>,
}

/// A wrapper for a avatar.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LuoxuAvatar(String);

impl LuoxuAvatar {
    pub fn new(avatar_uri: &MxcUri, homeserver: Url) -> Result<Self> {
        let (server_name, media_id) = avatar_uri.parts()?;
        let result = homeserver
            .join(format!("/_matrix/media/r0/download/{}/{}", server_name, media_id).as_str())?;
        Ok(LuoxuAvatar(result.to_string()))
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

/// A Event ID for Meilisearch primary key.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyEventId(String);

impl From<OwnedEventId> for KeyEventId {
    fn from(event_id: OwnedEventId) -> Self {
        KeyEventId(event_id.as_str().strip_prefix('$').unwrap().to_string())
    }
}

impl KeyEventId {
    pub fn event_id(&self) -> String {
        format!("${}", self.0)
    }
}

#[derive(Clone)]
/// State store backed by Heed.
pub struct HeedStore {
    pub env: heed::Env,
    pub index_db: heed::Database<Str, Str>,
    pub name_db: heed::Database<Str, Str>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct RoomInfo {
    pub index_name: String,
    pub room_name: Option<String>,
}

impl HeedStore {
    pub fn new(location: &str) -> Result<Self> {
        let env = EnvOpenOptions::new().max_dbs(2).open(location)?;
        let mut wtxn = env.write_txn()?;
        let index_db = env.create_database(&mut wtxn, Some("index"))?;
        let name_db = env.create_database(&mut wtxn, Some("name"))?;
        wtxn.commit()?;
        Ok(HeedStore {
            env,
            index_db,
            name_db,
        })
    }

    pub fn add_entry(&self, room_id: &str, index: &str, name: Option<&str>) -> Result<()> {
        let mut wtxn = self.env.write_txn()?;
        self.index_db.put(&mut wtxn, room_id, index)?;
        self.name_db
            .put(&mut wtxn, room_id, name.unwrap_or(room_id))?;
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

    pub fn update_entry(
        &self,
        room_id: &str,
        index: Option<&str>,
        name: Option<&str>,
    ) -> Result<()> {
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

    pub fn get_rooms(&self) -> Result<Vec<RoomInfo>> {
        let mut result = Vec::new();
        let rtxn = self.env.read_txn()?;
        let iter = self.index_db.iter(&rtxn)?;
        for item in iter {
            let (key, index_name) = item?;
            let room_name = self
                .name_db
                .get(&rtxn, key)?
                .map(|room_name| room_name.to_string());
            let info = RoomInfo {
                index_name: index_name.to_string(),
                room_name,
            };
            result.push(info);
        }
        Ok(result)
    }
}
