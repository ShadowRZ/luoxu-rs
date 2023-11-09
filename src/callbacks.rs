use anyhow::bail;
use anyhow::Ok;
use heed::types::Str;
use matrix_sdk::ruma::events::room::message::sanitize::HtmlSanitizerMode;
use matrix_sdk::ruma::events::room::message::sanitize::RemoveReplyFallback;
use matrix_sdk::ruma::events::room::message::MessageType;
use matrix_sdk::ruma::events::room::message::OriginalSyncRoomMessageEvent;
use matrix_sdk::ruma::events::room::message::Relation;
use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
use matrix_sdk::ruma::serde::Raw;
use matrix_sdk::ruma::OwnedRoomId;
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
        return Ok(());
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
        _ => return Ok(()),
    };
    let external_url = {
        use std::result::Result::Ok;
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
    let timestamp = ev.origin_server_ts;
    let room_id = room.room_id();
    let msg = LuoxuMessage {
        body,
        event_id: event_id.into(),
        external_url,
        user_id,
        user_display_name,
        timestamp,
        room_id: room_id.into(),
        ocr_body: None,
    };
    // Save message.
    let env = &ctx.env;
    let db = ctx.db;
    let search = &ctx.search;
    let index = get_index_value(room_id.into(), env.clone(), db)?;
    search
        .index(index)
        .add_or_update(&[msg], None::<&str>)
        .await?;
    Ok(())
}

fn get_index_value(
    room_id: OwnedRoomId,
    env: heed::Env,
    db: heed::Database<Str, Str>,
) -> anyhow::Result<String> {
    let rtxn = env.read_txn();
    use std::result::Result::Ok;
    match rtxn {
        Ok(rtxn) => match db.get(&rtxn, room_id.as_str()) {
            Ok(value) => {
                if let Some(value) = value {
                    Ok(value.to_string())
                } else {
                    bail!("No index matches the given room ID!")
                }
            }
            Err(e) => bail!("Getting element of state store failed: {}", e),
        },
        Err(e) => bail!("Getting transaction of state store failed: {}", e),
    }
}
