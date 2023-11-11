use anyhow::Context;
use luoxu_rs::LuoxuAvatar;
use matrix_sdk::ruma::events::room::message::sanitize::HtmlSanitizerMode;
use matrix_sdk::ruma::events::room::message::sanitize::RemoveReplyFallback;
use matrix_sdk::ruma::events::room::message::MessageType;
use matrix_sdk::ruma::events::room::message::OriginalSyncRoomMessageEvent;
use matrix_sdk::ruma::events::room::message::Relation;
use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
use matrix_sdk::ruma::events::room::name::OriginalSyncRoomNameEvent;
use matrix_sdk::ruma::events::room::tombstone::OriginalSyncRoomTombstoneEvent;
use matrix_sdk::ruma::serde::Raw;
use matrix_sdk::{
    event_handler::{Ctx, RawEvent},
    room::Room,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use luoxu_rs::{LuoxuBotContext, LuoxuMessage};

pub async fn on_room_message(
    ev: OriginalSyncRoomMessageEvent,
    room: Room,
    client: matrix_sdk::Client,
    ctx: Ctx<Arc<LuoxuBotContext>>,
    raw: RawEvent,
) -> anyhow::Result<()> {
    let user_id = ev.sender;
    // Stop processing our own messages.
    if user_id == client.user_id().unwrap() {
        return anyhow::Ok(());
    }
    // Gather event infomations.
    let value = raw.get().to_string();
    let raw: Raw<OriginalSyncRoomMessageEvent> = Raw::from_json_string(value)?;
    let (mut content, event_id) = match ev.content.relates_to {
        Some(Relation::Replacement(r)) => {
            let content: RoomMessageEventContent = *r.new_content;
            (content, r.event_id)
        }
        _ => (ev.content, ev.event_id),
    };
    content.sanitize(HtmlSanitizerMode::Strict, RemoveReplyFallback::Yes);
    let body = match content.msgtype {
        MessageType::Text(ev) => ev.body.trim_start().to_string(),
        MessageType::Image(ev) => format!("[Image] {}", ev.body),
        MessageType::File(ev) => format!("[File] {}", ev.body),
        MessageType::Video(ev) => format!("[Video] {}", ev.body),
        _ => return anyhow::Ok(()),
    };
    let external_url = {
        if let Ok(Some(content)) = raw.get_field::<HashMap<&str, _>>("content") {
            if let Some(Value::String(external_url)) = content.get("external_url") {
                Some(external_url.clone())
            } else {
                None
            }
        } else {
            None
        }
    };
    let user_display_name = {
        match room.get_member(&user_id).await? {
            Some(member) => member.display_name().map(|name| name.to_string()),
            None => None,
        }
    };
    let user_avatar = {
        match room.get_member(&user_id).await? {
            Some(member) => {
                let homeserver = client.homeserver().await;
                match member.avatar_url() {
                    Some(avatar_url) => Some(LuoxuAvatar::new(avatar_url, homeserver)?),
                    None => None,
                }
            }
            None => None,
        }
    };
    let timestamp = ev.origin_server_ts;
    let room_id = room.room_id();
    let msg = LuoxuMessage {
        body,
        event_id: event_id.into(),
        external_url,
        user_id,
        user_display_name,
        user_avatar,
        timestamp,
        room_id: room_id.into(),
        ocr_body: None,
    };
    // Save message.
    let search = &ctx.search;
    if let Ok(Some(index)) = &ctx.store.get_index(room_id.into()) {
        search
            .index(index)
            .add_or_update(&[msg], None::<&str>)
            .await?;
    }

    anyhow::Ok(())
}

pub async fn on_room_name(
    ev: OriginalSyncRoomNameEvent,
    room: Room,
    ctx: Ctx<Arc<LuoxuBotContext>>,
) -> anyhow::Result<()> {
    ctx.store
        .update_entry(room.room_id().as_str(), None, ev.content.name.as_deref())
}

pub async fn on_room_tombstone(
    ev: OriginalSyncRoomTombstoneEvent,
    room: Room,
    client: matrix_sdk::Client,
    ctx: Ctx<Arc<LuoxuBotContext>>,
) -> anyhow::Result<()> {
    tracing::info!(
        "Joining new room as a room replacement happened, event: {:#?}",
        ev
    );
    let _ = client
        .join_room_by_id(&ev.content.replacement_room)
        .await
        .context("Joining the new room failed")?;
    ctx.store.move_entry(
        room.room_id().as_str(),
        ev.content.replacement_room.as_str(),
    )?;
    Ok(())
}
