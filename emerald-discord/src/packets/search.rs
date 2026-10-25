//! Searching: the query strings of message, mention, pin and thread
//! searches, and the answers only those endpoints give.

use serde::Deserialize;

use crate::model::{Message, Snowflake};
use crate::protocol::{SearchQuery, ThreadQuery};
use crate::rest::encoding::encode_component;

/// Thread, pin and mention pages, as the web client loads them.
pub const PAGE: usize = 25;

/// The message search's query string. Keys repeat for lists, as the web
/// client's `qs.stringify` writes them; their order is a guess, since the
/// filters are gathered from the search box in whatever order they appear.
pub fn messages(query: &SearchQuery, offset: usize) -> String {
    let mut pairs: Vec<(&str, String)> = Vec::new();
    pairs.extend(query.authors.iter().map(|id| ("author_id", id.to_string())));
    pairs.extend(query.mentions.iter().map(|id| ("mentions", id.to_string())));
    pairs.extend(query.has.iter().map(|has| ("has", has.clone())));
    pairs.extend(query.min_id.map(|id| ("min_id", id.to_string())));
    pairs.extend(query.max_id.map(|id| ("max_id", id.to_string())));
    pairs.extend(query.channels.iter().map(|id| ("channel_id", id.to_string())));
    pairs.extend(query.pinned.map(|pinned| ("pinned", pinned.to_string())));
    if !query.content.trim().is_empty() {
        pairs.push(("content", query.content.trim().to_owned()));
    }
    if offset > 0 {
        pairs.push(("offset", offset.to_string()));
    }
    join(&pairs)
}

/// Recent mentions across every server, older than `before` when paging.
pub fn mentions(before: Option<Snowflake>) -> String {
    let mut pairs = Vec::new();
    pairs.extend(before.map(|id| ("before", id.to_string())));
    pairs.push(("limit", PAGE.to_string()));
    pairs.push(("roles", "true".into()));
    pairs.push(("everyone", "true".into()));
    join(&pairs)
}

/// Pins, pinned before `before` (an ISO timestamp) when paging.
pub fn pins(before: Option<&str>) -> String {
    let mut pairs = vec![("limit", PAGE.to_string())];
    pairs.extend(before.map(|at| ("before", at.to_owned())));
    join(&pairs)
}

/// A channel's threads: its archived ones newest first, or those whose
/// name matches. Tags narrow forum posts; any of them will do. Sorting by
/// creation as `creation_time` is a guess: the bundle only spells out
/// `last_message_time`.
pub fn threads(query: &ThreadQuery, offset: usize) -> String {
    let mut pairs = Vec::new();
    match &query.name {
        Some(name) => pairs.push(("name", name.clone())),
        None => pairs.push(("archived", "true".into())),
    }
    let sort = if query.by_creation { "creation_time" } else { "last_message_time" };
    pairs.push(("sort_by", sort.into()));
    pairs.push(("sort_order", "desc".into()));
    pairs.push(("limit", PAGE.to_string()));
    if !query.tags.is_empty() {
        let tags: Vec<String> = query.tags.iter().map(ToString::to_string).collect();
        pairs.push(("tag", tags.join(",")));
    }
    pairs.push(("tag_setting", "match_some".into()));
    pairs.push(("offset", offset.to_string()));
    join(&pairs)
}

fn join(pairs: &[(&str, String)]) -> String {
    pairs
        .iter()
        .map(|(key, value)| format!("{key}={}", encode_component(value)))
        .collect::<Vec<_>>()
        .join("&")
}

/// The answer to a message search: each hit is the first message of its
/// inner list. A 202 while Discord indexes carries `retry_after` and no
/// messages.
#[derive(Deserialize)]
pub struct SearchAnswer {
    #[serde(default)]
    pub messages: Vec<Vec<Message>>,
    #[serde(default)]
    pub total_results: u32,
    #[serde(default)]
    pub retry_after: Option<f64>,
}

/// The answer to a pin listing.
#[derive(Deserialize)]
pub struct PinsAnswer {
    #[serde(default)]
    pub items: Vec<PinnedItem>,
    #[serde(default)]
    pub has_more: bool,
}

#[derive(Deserialize)]
pub struct PinnedItem {
    pub message: Message,
    #[serde(default)]
    pub pinned_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_strings_encode_and_repeat_keys() {
        let query = SearchQuery {
            content: " olá mundo ".into(),
            authors: vec![Snowflake(1), Snowflake(2)],
            has: vec!["image".into()],
            ..SearchQuery::default()
        };
        assert_eq!(messages(&query, 25), "author_id=1&author_id=2&has=image&content=ol%C3%A1%20mundo&offset=25");
        assert_eq!(mentions(None), "limit=25&roles=true&everyone=true");
        assert_eq!(pins(Some("2024-01-01T00:00:00+00:00")), "limit=25&before=2024-01-01T00%3A00%3A00%2B00%3A00");
        let tagged = ThreadQuery {
            tags: vec![Snowflake(7), Snowflake(8)],
            ..ThreadQuery::default()
        };
        assert_eq!(
            threads(&tagged, 0),
            "archived=true&sort_by=last_message_time&sort_order=desc&limit=25&tag=7%2C8&tag_setting=match_some&offset=0"
        );
    }
}
