use serde::{Deserialize, Deserializer};

use super::{Member, Reaction, Snowflake, User};

#[derive(Clone, Debug, Deserialize)]
pub struct Attachment {
    pub filename: String,
    #[serde(default)]
    pub proxy_url: Option<String>,
    #[serde(default)]
    pub content_type: Option<String>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    /// Alt text.
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub flags: u64,
}

impl Attachment {
    /// Sent as a spoiler: flagged, or named the way older clients mark it.
    pub fn spoiler(&self) -> bool {
        const IS_SPOILER: u64 = 1 << 3;
        self.flags & IS_SPOILER != 0 || self.filename.starts_with("SPOILER_")
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct EmbedAuthor {
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct EmbedFooter {
    #[serde(default)]
    pub text: Option<String>,
}

/// Rich embed, reduced to its text parts.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Embed {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub color: Option<u32>,
    #[serde(default)]
    pub author: Option<EmbedAuthor>,
    #[serde(default)]
    pub footer: Option<EmbedFooter>,
    #[serde(default)]
    pub provider: Option<EmbedAuthor>,
}

pub mod message_kind {
    pub const DEFAULT: u8 = 0;
    pub const PIN: u8 = 6;
    pub const MEMBER_JOIN: u8 = 7;
    pub const BOOST: u8 = 8;
    pub const REPLY: u8 = 19;
}

pub mod sticker_format {
    pub const PNG: u8 = 1;
    pub const APNG: u8 = 2;
    /// Vector animation, the format of Discord's own sticker packs.
    pub const LOTTIE: u8 = 3;
    pub const GIF: u8 = 4;
}

#[derive(Debug, Deserialize)]
pub struct StickerItem {
    pub id: Snowflake,
    pub name: String,
    pub format_type: u8,
}

impl StickerItem {
    /// Where the sticker is downloaded from, or `None` for formats newer
    /// than this client. APNG decodes as its first frame. Lottie files come
    /// only from the CDN: the media proxy does not serve them.
    pub fn url(&self) -> Option<String> {
        let ext = match self.format_type {
            sticker_format::PNG | sticker_format::APNG => "png",
            sticker_format::GIF => "gif",
            sticker_format::LOTTIE => return Some(format!("https://cdn.discordapp.com/stickers/{}.json", self.id)),
            _ => return None,
        };
        Some(format!("https://media.discordapp.net/stickers/{}.{ext}?size=160", self.id))
    }

    pub fn is_lottie(&self) -> bool {
        self.format_type == sticker_format::LOTTIE
    }
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
    #[serde(default)]
    pub sticker_items: Vec<StickerItem>,
    #[serde(default)]
    pub reactions: Vec<Reaction>,
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
}

#[derive(Debug, Deserialize)]
pub struct MessageDelete {
    pub id: Snowflake,
    pub channel_id: Snowflake,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sticker_urls_by_format() {
        let sticker = |format_type| StickerItem {
            id: Snowflake(7),
            name: "wave".into(),
            format_type,
        };
        assert_eq!(
            sticker(sticker_format::GIF).url().as_deref(),
            Some("https://media.discordapp.net/stickers/7.gif?size=160")
        );
        assert_eq!(
            sticker(sticker_format::LOTTIE).url().as_deref(),
            Some("https://cdn.discordapp.com/stickers/7.json")
        );
        assert_eq!(sticker(9).url(), None);
    }

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
