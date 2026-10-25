//! What a message points at (a reply or a forward), and forwarded copies.

use serde::Deserialize;

use super::{Attachment, Embed, StickerItem};
use crate::model::{Snowflake, User};

/// What a message points at: the message it replies to, or the one it
/// forwards.
#[derive(Clone, Debug, Deserialize)]
pub struct MessageReference {
    /// 0 a reply, 1 a forward.
    #[serde(rename = "type", default)]
    pub kind: u8,
    #[serde(default)]
    pub message_id: Option<Snowflake>,
    #[serde(default)]
    pub channel_id: Option<Snowflake>,
    #[serde(default)]
    pub guild_id: Option<Snowflake>,
}

pub const REFERENCE_FORWARD: u8 = 1;

/// A copy of a forwarded message, as it was when forwarded.
#[derive(Clone, Debug, Deserialize)]
pub struct MessageSnapshot {
    pub message: SnapshotMessage,
}

/// The parts of a message a forward carries.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct SnapshotMessage {
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub timestamp: Option<String>,
    #[serde(default)]
    pub mentions: Vec<User>,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    #[serde(default)]
    pub embeds: Vec<Embed>,
    #[serde(default)]
    pub sticker_items: Vec<StickerItem>,
}
