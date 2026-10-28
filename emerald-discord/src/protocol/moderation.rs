//! What a moderator asks for in a channel's settings. Each field is
//! optional: a save sends only the ones the user changed.

use crate::model::Snowflake;

/// A channel's or thread's settings as the settings dialog saves them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChannelEdit {
    pub name: Option<String>,
    pub topic: Option<String>,
    /// Seconds between one member's messages; 0 turns slow mode off.
    pub slow_mode: Option<u32>,
    /// Forum tags as they should end up: the whole list, not a diff.
    pub tags: Option<Vec<TagEdit>>,
    /// Threads only: archive or unarchive.
    pub archived: Option<bool>,
    /// Threads only: lock (only moderators post) or unlock.
    pub locked: Option<bool>,
}

/// One forum tag of a [`ChannelEdit`]. A tag without an `id` is new.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagEdit {
    pub id: Option<Snowflake>,
    pub name: String,
    /// Only those who manage threads may apply it.
    pub moderated: bool,
    pub emoji_id: Option<Snowflake>,
    pub emoji_name: Option<String>,
}
