//! Values the UI fills for searches and settings changes, before they are
//! turned into requests.

use crate::model::{Notify, Snowflake};

/// A change to the user's notification settings for one server, or for
/// the DMs when its guild is `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsChange {
    /// Mutes the server, or `channel` in it (a category included), for
    /// `seconds`; -1 lasts until turned off. `None` unmutes.
    Mute {
        channel: Option<Snowflake>,
        seconds: Option<i64>,
    },
    /// What the server, or `channel`, notifies of.
    Notify {
        channel: Option<Snowflake>,
        level: Notify,
    },
    /// A category folded or unfolded in the sidebar.
    Collapse { category: Snowflake, collapsed: bool },
    HideMuted(bool),
    SuppressEveryone(bool),
    SuppressRoles(bool),
}

/// The mute lengths the web client offers, in seconds; -1 for "Until I
/// turn it back on".
pub const MUTE_WINDOWS: [i64; 6] = [900, 3600, 10_800, 28_800, 86_400, -1];

/// Where to search messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SearchScope {
    Guild(Snowflake),
    /// A DM, or one channel when searching a guild is not possible.
    Channel(Snowflake),
}

/// A message search, as the search box's filters fill it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchQuery {
    pub content: String,
    pub authors: Vec<Snowflake>,
    pub mentions: Vec<Snowflake>,
    pub channels: Vec<Snowflake>,
    /// `link`, `embed`, `file`, `image`, `video`, `sound`, `sticker`, `poll`.
    pub has: Vec<String>,
    /// Snowflakes made from the `after:` and `before:` dates.
    pub min_id: Option<Snowflake>,
    pub max_id: Option<Snowflake>,
    pub pinned: Option<bool>,
}

impl SearchQuery {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// How a channel's threads are listed and narrowed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ThreadQuery {
    /// Only threads whose name holds this.
    pub name: Option<String>,
    /// Only posts with one of these tags.
    pub tags: Vec<Snowflake>,
    /// Newest first by creation instead of by activity.
    pub by_creation: bool,
}
