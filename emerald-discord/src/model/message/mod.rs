//! Messages and what they carry: [`attachment`]s, [`embed`]s, [`poll`]s,
//! [`sticker`]s and [`reference`]s to other messages.

mod attachment;
mod embed;
mod gif;
mod poll;
mod reference;
mod sticker;

use serde::{Deserialize, Deserializer};

pub use attachment::*;
pub use embed::*;
pub use gif::*;
pub use poll::*;
pub use reference::*;
pub use sticker::*;

use super::{Member, Reaction, Snowflake, User};

pub mod message_flags {
    /// A voice message: one audio attachment with a waveform.
    pub const IS_VOICE_MESSAGE: u64 = 1 << 13;
}

pub mod message_kind {
    pub const DEFAULT: u8 = 0;
    pub const PIN: u8 = 6;
    pub const MEMBER_JOIN: u8 = 7;
    pub const BOOST: u8 = 8;
    pub const REPLY: u8 = 19;
}

#[derive(Debug, Deserialize)]
pub struct Message {
    pub id: Snowflake,
    pub channel_id: Snowflake,
    /// `message_kind`: 0 default, 19 reply, others system messages.
    #[serde(rename = "type", default)]
    pub kind: u8,
    /// Absent on DM messages.
    #[serde(default)]
    pub guild_id: Option<Snowflake>,
    pub author: User,
    /// The author's guild membership; gateway guild messages only.
    #[serde(default)]
    pub member: Option<Member>,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub mentions: Vec<User>,
    #[serde(default)]
    pub mention_roles: Vec<Snowflake>,
    #[serde(default)]
    pub mention_everyone: bool,
    #[serde(default)]
    pub pinned: bool,
    /// Set once the author edited the message.
    #[serde(default)]
    pub edited_timestamp: Option<String>,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    #[serde(default)]
    pub embeds: Vec<Embed>,
    /// The message this one replies to; `None` inside or when deleted.
    #[serde(default)]
    pub referenced_message: Option<Box<Message>>,
    /// The thread this message started.
    #[serde(default)]
    pub thread: Option<Box<super::Channel>>,
    #[serde(default)]
    pub sticker_items: Vec<StickerItem>,
    #[serde(default)]
    pub reactions: Vec<Reaction>,
    /// `message_flags`.
    #[serde(default)]
    pub flags: u64,
    #[serde(default)]
    pub message_reference: Option<MessageReference>,
    /// The forwarded message, on a forward.
    #[serde(default)]
    pub message_snapshots: Vec<MessageSnapshot>,
    #[serde(default)]
    pub poll: Option<Poll>,
    /// Echo of the nonce the sender chose; matches our pending copy.
    #[serde(default, deserialize_with = "lenient_nonce")]
    pub nonce: Option<String>,
}

/// Clients send nonces as strings or numbers; anything else is ignored.
fn lenient_nonce<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(match serde_json::Value::deserialize(d)? {
        serde_json::Value::String(s) => Some(s),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    })
}

/// MESSAGE_UPDATE carries only what changed: an edit brings the content,
/// a pin the flag, a link preview the embeds.
#[derive(Debug, Deserialize)]
pub struct MessageUpdate {
    pub id: Snowflake,
    pub channel_id: Snowflake,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub edited_timestamp: Option<String>,
    #[serde(default)]
    pub pinned: Option<bool>,
    #[serde(default)]
    pub embeds: Option<Vec<Embed>>,
    #[serde(default)]
    pub mentions: Option<Vec<User>>,
    #[serde(default)]
    pub mention_roles: Option<Vec<Snowflake>>,
    #[serde(default)]
    pub mention_everyone: Option<bool>,
    /// Set when a thread was started from the message.
    #[serde(default)]
    pub thread: Option<Box<super::Channel>>,
    /// A poll that ended comes back with its final results.
    #[serde(default)]
    pub poll: Option<Poll>,
}

#[derive(Debug, Deserialize)]
pub struct MessageDelete {
    pub id: Snowflake,
    pub channel_id: Snowflake,
}

/// MESSAGE_DELETE_BULK: several messages of one channel removed at once.
#[derive(Debug, Deserialize)]
pub struct MessageDeleteBulk {
    pub channel_id: Snowflake,
    pub ids: Vec<Snowflake>,
}

#[derive(Debug, Deserialize)]
pub struct MessageAck {
    pub channel_id: Snowflake,
    pub message_id: Snowflake,
    /// Mentions left unread; only "mark unread" sets it.
    #[serde(default)]
    pub mention_count: u32,
}

#[derive(Debug, Deserialize)]
pub struct TypingStart {
    pub channel_id: Snowflake,
    pub user_id: Snowflake,
    /// In guilds: the typist's member, with their user and nickname.
    #[serde(default)]
    pub guild_id: Option<Snowflake>,
    #[serde(default)]
    pub member: Option<Member>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonce_accepts_strings_and_numbers() {
        let m = |nonce: &str| {
            serde_json::from_str::<Message>(&format!(
                r#"{{"id":"1","channel_id":"2","author":{{"id":"3","username":"a"}},"timestamp":"","nonce":{nonce}}}"#
            ))
            .unwrap()
            .nonce
        };
        assert_eq!(m(r#""42""#).as_deref(), Some("42"));
        assert_eq!(m("42").as_deref(), Some("42"));
        assert_eq!(m("null"), None);
    }
}
