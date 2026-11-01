//! Bookmarks and reminders, kept by Discord across devices.

use anyhow::Result;

use super::Rest;
use crate::model::{BookmarkPage, SavedList, SavedMessage, Snowflake};
use crate::packets::saved::{self, SaveSource};
use emerald_http::Call;

impl Rest {
    /// Every reminder, and the ids of the bookmarked messages.
    pub async fn saved_messages(&self) -> Result<SavedList> {
        self.api
            .fetch(Call::get("/users/@me/saved-messages", "/users/@me/saved-messages?bookmark_ids=true"))
            .await
    }

    /// 25 bookmarks saved before `before`, a `saved_at` string; the first
    /// page leaves it out.
    pub async fn bookmarks(&self, before: Option<&str>) -> Result<BookmarkPage> {
        let mut query = String::new();
        if let Some(before) = before {
            query.push_str("before=");
            query.push_str(&super::encoding::encode_component(before));
            query.push('&');
        }
        query.push_str("limit=25");
        let path = format!("/users/@me/saved-messages/bookmarks?{query}");
        self.api.fetch(Call::get("/users/@me/saved-messages/bookmarks", path)).await
    }

    /// Bookmarks a message, or sets (and moves) its reminder.
    pub async fn save_message(
        &self,
        channel: Snowflake,
        message: Snowflake,
        due_secs: Option<i64>,
        source: SaveSource,
    ) -> Result<SavedMessage> {
        let path = format!("/users/@me/saved-messages/{channel}/{message}");
        self.api
            .fetch(Call::put("/users/@me/saved-messages/{id}/{id}", path).json(saved::save(due_secs, source)))
            .await
    }

    /// Removes a bookmark or a reminder; also "Mark Complete".
    pub async fn unsave_message(&self, channel: Snowflake, message: Snowflake) -> Result<()> {
        let path = format!("/users/@me/saved-messages/{channel}/{message}");
        self.api.run(Call::delete("/users/@me/saved-messages/{id}/{id}", path)).await
    }
}
