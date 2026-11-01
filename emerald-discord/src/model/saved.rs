//! Saved messages: bookmarks and reminders. One server object serves both;
//! a reminder is a saved message with `due_at`.

use serde::{Deserialize, Deserializer};

use super::{Message, Snowflake, iso_secs};

/// A bookmark or a reminder, with the message when it can still be read.
#[derive(Debug, Deserialize)]
pub struct SavedMessage {
    #[serde(default)]
    pub message: Option<Message>,
    pub save_data: SaveData,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SaveData {
    pub channel_id: Snowflake,
    pub message_id: Snowflake,
    #[serde(default)]
    pub saved_at: String,
    /// Discord sends the number 0 for none.
    #[serde(default, deserialize_with = "zero_is_none")]
    pub guild_id: Option<Snowflake>,
    #[serde(default, deserialize_with = "zero_is_none")]
    pub author_id: Option<Snowflake>,
    #[serde(default)]
    pub due_at: Option<String>,
    #[serde(default)]
    pub author_summary: Option<String>,
    #[serde(default)]
    pub channel_summary: Option<String>,
    #[serde(default)]
    pub message_summary: Option<String>,
}

impl SavedMessage {
    /// When the reminder is due, in Unix seconds; `None` for a bookmark.
    pub fn due_secs(&self) -> Option<i64> {
        iso_secs(self.save_data.due_at.as_deref()?)
    }

    pub fn is_reminder(&self) -> bool {
        self.save_data.due_at.is_some()
    }
}

/// Answer to `GET /users/@me/saved-messages?bookmark_ids=true`: every
/// reminder in full, bookmarks only as message ids.
#[derive(Debug, Default, Deserialize)]
pub struct SavedList {
    #[serde(default)]
    pub reminders: Vec<SavedMessage>,
    #[serde(default)]
    pub bookmark_ids: Vec<Snowflake>,
}

/// A page of bookmarks, newest first.
#[derive(Debug, Default, Deserialize)]
pub struct BookmarkPage {
    #[serde(default)]
    pub results: Vec<SavedMessage>,
    #[serde(default)]
    pub has_more: bool,
}

/// `SAVED_MESSAGE_DELETE`.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct SavedMessageRemoved {
    pub channel_id: Snowflake,
    pub message_id: Snowflake,
}

fn zero_is_none<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Snowflake>, D::Error> {
    Ok(Option::<Snowflake>::deserialize(d)?.filter(|id| id.0 != 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_ids_are_none_and_due_parses() {
        let saved: SavedMessage = serde_json::from_str(
            r#"{"message":null,"save_data":{"channel_id":"10","message_id":"20",
            "saved_at":"2026-10-30T11:00:00.000000+00:00","guild_id":0,"author_id":"30",
            "notes":"","due_at":"2026-10-30T12:00:00+00:00","author_summary":"a",
            "channel_summary":"c","message_summary":"m"}}"#,
        )
        .unwrap();
        assert_eq!(saved.save_data.guild_id, None);
        assert_eq!(saved.save_data.author_id, Some(Snowflake(30)));
        assert!(saved.is_reminder());
        assert_eq!(saved.due_secs(), Some(1_793_361_600));
    }

    #[test]
    fn list_and_page_parse() {
        let list: SavedList =
            serde_json::from_str(r#"{"reminders":[],"bookmark_ids":["1","2"]}"#).unwrap();
        assert_eq!(list.bookmark_ids, [Snowflake(1), Snowflake(2)]);
        let page: BookmarkPage = serde_json::from_str(r#"{"results":[],"has_more":true}"#).unwrap();
        assert!(page.has_more);
    }
}
