use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use axum_macros::debug_handler;
use luoxu_rs::{LuoxuBotContext, LuoxuMessage, RoomInfo};
use meilisearch_sdk::Selectors;
use ruma::MilliSecondsSinceUnixEpoch;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;

type RouteResult<T> = Result<T, AppError>;

/// List all groups indexed.
pub async fn groups(State(state): State<Arc<LuoxuBotContext>>) -> RouteResult<Json<Vec<RoomInfo>>> {
    let result = state.store.get_rooms()?;
    Ok(Json(result))
}

/// Search a group.
/// GET /search/:index_name?query=
#[debug_handler]
pub async fn group_search(
    State(state): State<Arc<LuoxuBotContext>>,
    Path(index_name): Path<String>,
    Query(params): Query<Params>,
) -> RouteResult<Json<MessageSearchResults>> {
    #[allow(unused_assignments)]
    let mut filter: String = "".to_string();
    let index = state.search.index(index_name);
    let mut query = index.search();
    let mut query = query
        .with_query(&params.query)
        .with_sort(&["timestamp:desc"])
        .with_attributes_to_search_on(&["body"])
        .with_attributes_to_highlight(Selectors::Some(&["body"]))
        .with_highlight_pre_tag("<span class=\"keyword\"")
        .with_highlight_post_tag("</span>");
    if let Some(offset) = params.offset {
        let offset = offset.0;
        filter = format!("timestamp < {}", offset);
        query = query.with_filter(&filter);
    }
    let search_result = query.execute::<LuoxuMessage>().await?;
    let result = MessageSearchResults {
        messages: search_result
            .hits
            .iter()
            .map(|item| {
                let result = item.result.clone();
                let formatted_result = item.formatted_result.as_ref().unwrap();
                let highlight_body = formatted_result.get("body").unwrap();
                MessageSearchResult {
                    event_id: result.event_id.event_id(),
                    html_body: highlight_body.to_string(),
                    external_url: result.external_url,
                    display_name: result.user_display_name,
                    timestamp: result.timestamp,
                    room_id: result.room_id.to_string(),
                    avatar_url: result.user_avatar.map(|result| result.into_string()),
                }
            })
            .collect(),
        has_more: search_result.estimated_total_hits.unwrap() > search_result.limit.unwrap(),
    };
    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
pub struct Params {
    query: String,
    offset: Option<MilliSecondsSinceUnixEpoch>,
}

// Make our own error that wraps `anyhow::Error`.
pub struct AppError(anyhow::Error);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self.0),
        )
            .into_response()
    }
}

// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageSearchResults {
    pub messages: Vec<MessageSearchResult>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageSearchResult {
    pub event_id: String, // Primary
    pub html_body: String,
    pub external_url: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub timestamp: MilliSecondsSinceUnixEpoch,
    pub room_id: String,
}
