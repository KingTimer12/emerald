//! GUILD_MEMBER_LIST_UPDATE: the lazily loaded member sidebar. The server
//! keeps a virtual array of group headers and members per list and sends
//! operations on it for the row ranges the client subscribed to.

use serde::Deserialize;

use super::{Snowflake, User};

#[derive(Debug, Deserialize)]
pub struct MemberListUpdate {
    /// `everyone`, or a hash of the overwrites deciding who sees the channel.
    pub id: String,
    pub guild_id: Snowflake,
    #[serde(default)]
    pub member_count: u64,
    #[serde(default)]
    pub online_count: u64,
    /// Every non-empty group in list order; rows are each group's header
    /// followed by its members.
    #[serde(default)]
    pub groups: Vec<MemberListGroup>,
    #[serde(default)]
    pub ops: Vec<MemberListOp>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MemberListGroup {
    /// A hoisted role's id, or `online` / `offline`.
    pub id: String,
    #[serde(default)]
    pub count: u64,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "UPPERCASE")]
pub enum MemberListOp {
    Sync {
        range: [usize; 2],
        #[serde(default)]
        items: Vec<MemberListItem>,
    },
    Insert {
        index: usize,
        item: MemberListItem,
    },
    Update {
        index: usize,
        item: MemberListItem,
    },
    Delete {
        index: usize,
    },
    Invalidate {
        range: [usize; 2],
    },
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemberListItem {
    Group(MemberListGroup),
    Member(Box<ListMember>),
}

#[derive(Clone, Debug, Deserialize)]
pub struct ListMember {
    pub user: User,
    #[serde(default)]
    pub nick: Option<String>,
    #[serde(default)]
    pub roles: Vec<Snowflake>,
    #[serde(default)]
    pub presence: Option<ListPresence>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ListPresence {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub activities: Vec<Activity>,
}

pub mod activity_kind {
    pub const PLAYING: u8 = 0;
    pub const STREAMING: u8 = 1;
    pub const LISTENING: u8 = 2;
    pub const WATCHING: u8 = 3;
    pub const CUSTOM: u8 = 4;
    pub const COMPETING: u8 = 5;
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Activity {
    #[serde(rename = "type", default)]
    pub kind: u8,
    #[serde(default)]
    pub name: String,
    /// Custom status text, the party state of a game, or the artist on
    /// Spotify.
    #[serde(default)]
    pub state: Option<String>,
    /// What the game says the player is doing, or the song on Spotify.
    #[serde(default)]
    pub details: Option<String>,
    /// The custom status's emoji.
    #[serde(default)]
    pub emoji: Option<ActivityEmoji>,
    #[serde(default)]
    pub timestamps: Option<ActivityTimestamps>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ActivityEmoji {
    /// The Unicode emoji, or the custom one's name.
    pub name: String,
    #[serde(default)]
    pub id: Option<Snowflake>,
}

/// Unix milliseconds.
#[derive(Clone, Copy, Debug, Default, Deserialize)]
pub struct ActivityTimestamps {
    #[serde(default)]
    pub start: Option<u64>,
    #[serde(default)]
    pub end: Option<u64>,
}

impl Activity {
    /// The custom status's text with its emoji in front, when the emoji is
    /// a Unicode one.
    pub fn custom_text(&self) -> Option<String> {
        let text = self.state.as_deref().map(str::trim).filter(|s| !s.is_empty());
        let emoji = self.emoji.as_ref().filter(|e| e.id.is_none()).map(|e| e.name.as_str());
        match (emoji, text) {
            (Some(emoji), Some(text)) => Some(format!("{emoji} {text}")),
            (Some(emoji), None) => Some(emoji.to_owned()),
            (None, text) => text.map(str::to_owned),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sync_and_unknown_ops() {
        let update: MemberListUpdate = serde_json::from_str(
            r#"{"id":"everyone","guild_id":"1","member_count":3,"online_count":2,
                "groups":[{"id":"online","count":2}],
                "ops":[{"op":"SYNC","range":[0,99],"items":[
                    {"group":{"id":"online","count":2}},
                    {"member":{"user":{"id":"5","username":"ana"},"roles":["7"],
                        "presence":{"status":"idle","activities":[{"type":4,"name":"Custom Status","state":"oi"}]}}}
                ]},{"op":"FUTURE"}]}"#,
        )
        .unwrap();
        let MemberListOp::Sync { items, .. } = &update.ops[0] else { panic!() };
        let MemberListItem::Member(member) = &items[1] else { panic!() };
        assert_eq!(member.presence.as_ref().unwrap().activities[0].state.as_deref(), Some("oi"));
        assert!(matches!(update.ops[1], MemberListOp::Unknown));
    }
}
