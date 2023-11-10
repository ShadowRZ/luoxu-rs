use anyhow::bail;
use luoxu_rs::LuoxuBotContext;
use luoxu_rs::LuoxuConfig;
use matrix_sdk::config::SyncSettings;
use matrix_sdk::ruma::{RoomAliasId, RoomId};
use matrix_sdk::Session;
use meilisearch_sdk::IndexesQuery;

use std::sync::Arc;

use crate::callbacks::on_room_message;
use crate::callbacks::on_room_name;
use crate::callbacks::on_room_tombstone;

pub enum LoginType {
    Password(String),
    Session(Session),
}

pub struct LuoxuBot {
    config: LuoxuConfig,
    client: matrix_sdk::Client,
    context: Arc<LuoxuBotContext>,
}

impl LuoxuBot {
    pub async fn new(config: LuoxuConfig) -> anyhow::Result<Self> {
        use matrix_sdk::Client;
        let builder = Client::builder()
            .homeserver_url(&config.matrix.homeserver_url)
            .sled_store("store", None)?;
        let client = builder.build().await?;

        let context = config.get_context()?;

        Ok(LuoxuBot {
            config,
            client,
            context: context.into(),
        })
    }

    pub async fn login(&self, login: LoginType) -> anyhow::Result<Option<Session>> {
        match login {
            LoginType::Password(password) => {
                let response = self
                    .client
                    .login_username(&self.config.matrix.username, &password)
                    .initial_device_display_name(&self.config.matrix.device_name)
                    .send()
                    .await?;
                let session: Session = response.into();
                Ok(Some(session))
            }
            LoginType::Session(session) => {
                self.client.restore_login(session).await?;
                Ok(None)
            }
        }
    }

    pub async fn update_indices(&self) -> anyhow::Result<()> {
        let client = &self.context.search;
        let indices = IndexesQuery::new(client).with_limit(512).execute().await?;
        let result = self.config.matrix.indices.iter();
        for (index, _) in result {
            let created: Vec<_> = indices.results.iter().map(|i| i.uid.clone()).collect();
            if !created.contains(&index.to_string()) {
                let index = client
                    .create_index(index, Some("event_id"))
                    .await?
                    .wait_for_completion(client, None, None)
                    .await?
                    .try_make_index(client)
                    .unwrap();
                index
                    .set_filterable_attributes(&["user_id"])
                    .await?
                    .wait_for_completion(client, None, None)
                    .await?;
                index
                    .set_sortable_attributes(&["timestamp"])
                    .await?
                    .wait_for_completion(client, None, None)
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn update_state(&self) -> anyhow::Result<()> {
        let indcies = &self.config.matrix.indices;
        for (index, room) in indcies {
            let room = if room.starts_with('#') {
                let room_alias = <&RoomAliasId>::try_from(room.as_str())?;
                let room_id = self.client.resolve_room_alias(room_alias).await?.room_id;
                room_id.to_string()
            } else {
                room.to_string()
            };
            if let Some(room) = self.client.get_room(<&RoomId>::try_from(room.as_str())?) {
                let name = room.name();
                if let Err(e) = self.context.store.update_entry(
                    room.room_id().as_str(),
                    Some(index),
                    name.as_deref(),
                ) {
                    bail!("Updating state failed: {}", e)
                }
            }
        }
        Ok(())
    }

    pub async fn run(self) -> anyhow::Result<()> {
        self.client.add_event_handler_context(self.context.clone());
        tracing::info!("Initial sync beginning...");
        self.client.sync_once(SyncSettings::default()).await?;
        self.client.add_event_handler(on_room_message);
        self.client.add_event_handler(on_room_name);
        self.client.add_event_handler(on_room_tombstone);
        let settings = SyncSettings::default().token(self.client.sync_token().await.unwrap());
        self.client.sync(settings).await?;
        Ok(())
    }
}
