//! The content inventory feed: what friends and server mates played,
//! watched and listened to lately, behind the member list's Activity
//! section. Entries are kept loosely: only the fields the section uses
//! are typed, and an entry that does not parse is left out.

use serde::Deserialize;

use super::{Snowflake, lenient_list};

/// `content_type` values, as the web client names them.
pub mod content_type {
    pub const PLAYED_GAME: u32 = 1;
    pub const WATCHED_MEDIA: u32 = 2;
    pub const TOP_GAME: u32 = 3;
    pub const LISTENED_MEDIA: u32 = 4;
    pub const LISTENED_SESSION: u32 = 5;
    pub const TOP_ARTIST: u32 = 6;
    pub const CUSTOM_STATUS: u32 = 7;
    pub const LAUNCHED_ACTIVITY: u32 = 8;
    pub const LEADERBOARD: u32 = 9;
}

/// Trait types the cards read.
pub mod trait_type {
    pub const DURATION_SECONDS: u32 = 2;
    pub const IS_LIVE: u32 = 3;
    pub const AGGREGATE_COUNT: u32 = 12;
}

/// `GET /content-inventory/users/@me`.
#[derive(Debug, Default, Deserialize)]
pub struct Feed {
    #[serde(default, deserialize_with = "lenient_list")]
    pub entries: Vec<FeedEntry>,
    #[serde(default)]
    pub refresh_token: Option<String>,
    /// How long to wait before asking again.
    #[serde(default)]
    pub wait_ms_until_next_fetch: Option<u64>,
}

/// One wrapper of `entries`: the content itself.
#[derive(Debug, Deserialize)]
pub struct FeedEntry {
    pub content: Content,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Content {
    pub id: String,
    pub author_id: Snowflake,
    pub content_type: u32,
    /// What the card names: a game, a show, an artist… Its fields are
    /// read by name, since the bundle research did not cover their shape.
    #[serde(default)]
    pub extra: serde_json::Value,
    #[serde(default)]
    pub traits: Vec<serde_json::Value>,
}

impl Content {
    /// A trait's field, by trait type.
    pub fn trait_value(&self, kind: u32, field: &str) -> Option<&serde_json::Value> {
        self.traits
            .iter()
            .find(|t| t.get("type").and_then(serde_json::Value::as_u64) == Some(u64::from(kind)))
            .and_then(|t| t.get(field))
    }

    /// The first name-like field of `extra`: what the entry is about.
    /// A guess at the shape, see [`Content::extra`].
    pub fn subject(&self) -> Option<&str> {
        ["game_name", "application_name", "media_title", "title", "artist_name", "name"]
            .iter()
            .find_map(|field| self.extra.get(*field).and_then(serde_json::Value::as_str))
    }
}

/// `CONTENT_INVENTORY_INBOX_STALE`.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct InboxStale {
    #[serde(default)]
    pub refresh_after_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_content_types_and_broken_entries_still_parse() {
        let feed: Feed = serde_json::from_str(
            r#"{"entries":[
                {"content":{"id":"a","author_id":"1","content_type":1,"extra":{"game_name":"Celeste"},
                 "traits":[{"type":2,"duration_seconds":3600}]}},
                {"content":{"id":"b","author_id":"2","content_type":77}},
                {"content":{"id":"c"}}
            ],"refresh_token":"t","wait_ms_until_next_fetch":60000}"#,
        )
        .unwrap();
        assert_eq!(feed.entries.len(), 2);
        let first = &feed.entries[0].content;
        assert_eq!(first.subject(), Some("Celeste"));
        assert_eq!(
            first.trait_value(trait_type::DURATION_SECONDS, "duration_seconds").and_then(|v| v.as_u64()),
            Some(3600)
        );
        assert_eq!(feed.refresh_token.as_deref(), Some("t"));
    }
}
