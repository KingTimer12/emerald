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

/// A sticker on a message, or one a guild uploaded.
#[derive(Clone, Debug, Deserialize)]
pub struct StickerItem {
    pub id: Snowflake,
    pub name: String,
    pub format_type: u8,
    /// Words the sticker is found by in the picker; guild stickers only.
    #[serde(default)]
    pub tags: String,
    /// False once the guild lost the boost level that unlocked it.
    #[serde(default = "available")]
    pub available: bool,
}

fn available() -> bool {
    true
}

/// One of Discord's standard sticker packs, which everyone may send from.
#[derive(Clone, Debug, Deserialize)]
pub struct StickerPack {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub stickers: Vec<StickerItem>,
}

/// `GET /sticker-packs`.
#[derive(Debug, Deserialize)]
pub struct StickerPacks {
    #[serde(default)]
    pub sticker_packs: Vec<StickerPack>,
}

/// GUILD_STICKERS_UPDATE: the guild's whole list.
#[derive(Debug, Deserialize)]
pub struct GuildStickersUpdate {
    pub guild_id: Snowflake,
    #[serde(default)]
    pub stickers: Vec<StickerItem>,
}

/// A GIF from Discord's Tenor proxy, as the GIF picker lists them.
#[derive(Clone, Debug, Deserialize)]
pub struct Gif {
    /// Tenor's id, a string of digits.
    #[serde(deserialize_with = "text_or_number")]
    pub id: String,
    /// The Tenor page; sending it is how a GIF is posted.
    pub url: String,
    /// A still of the GIF; not every answer has one.
    #[serde(default)]
    pub preview: Option<String>,
    /// The GIF itself, in GIF format.
    #[serde(default)]
    pub gif_src: Option<String>,
    /// The GIF in the asked `media_format`, usually a video.
    #[serde(default)]
    pub src: Option<String>,
}

impl Gif {
    /// The first of its URLs that is an image this client can draw, as
    /// the web client falls back from the video `src` to `gif_src`.
    pub fn image_url(&self) -> Option<&str> {
        [&self.preview, &self.gif_src, &self.src]
            .into_iter()
            .filter_map(|url| url.as_deref())
            .find(|url| is_image_url(url))
    }
}

/// Whether `url`'s path ends like a PNG, JPEG, GIF or WebP file.
fn is_image_url(url: &str) -> bool {
    let path = url.split(['?', '#']).next().unwrap_or_default().to_ascii_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".webp"].iter().any(|ext| path.ends_with(ext))
}

fn text_or_number<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    match serde_json::Value::deserialize(deserializer)? {
        serde_json::Value::String(text) => Ok(text),
        serde_json::Value::Number(number) => Ok(number.to_string()),
        other => Err(serde::de::Error::custom(format!("unexpected GIF id {other}"))),
    }
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
    /// The thread this message started.
    #[serde(default)]
    pub thread: Option<Box<super::Channel>>,
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
    /// Set when a thread was started from the message.
    #[serde(default)]
    pub thread: Option<Box<super::Channel>>,
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
    fn sticker_urls_by_format() {
        let sticker = |format_type| StickerItem {
            id: Snowflake(7),
            name: "wave".into(),
            format_type,
            tags: String::new(),
            available: true,
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

    #[test]
    fn gifs_and_guild_stickers_parse() {
        let gif: Gif = serde_json::from_str(
            r#"{"id":"123","title":"","url":"https://tenor.com/view/x-123","src":"https://media.tenor.com/a.webm",
                "gif_src":"https://media.tenor.com/a.gif","width":498,"height":280,"preview":"https://media.tenor.com/a.png"}"#,
        )
        .unwrap();
        assert_eq!((gif.id.as_str(), gif.preview.as_deref()), ("123", Some("https://media.tenor.com/a.png")));
        assert_eq!(gif.image_url(), Some("https://media.tenor.com/a.png"));
        let gif: Gif = serde_json::from_str(r#"{"id":7,"url":"u"}"#).unwrap();
        assert_eq!((gif.id.as_str(), gif.image_url()), ("7", None));
        // A video preview is passed over for the GIF.
        let gif: Gif = serde_json::from_str(
            r#"{"id":"1","url":"u","src":"https://m/a.webm","preview":"https://m/a.mp4","gif_src":"https://m/a.GIF?x=1"}"#,
        )
        .unwrap();
        assert_eq!(gif.image_url(), Some("https://m/a.GIF?x=1"));
        let update: GuildStickersUpdate = serde_json::from_str(
            r#"{"guild_id":"1","stickers":[{"id":"2","name":"wave","tags":"hello","format_type":1,"available":false}]}"#,
        )
        .unwrap();
        let sticker = &update.stickers[0];
        assert_eq!((sticker.tags.as_str(), sticker.available), ("hello", false));
        let packs: StickerPacks = serde_json::from_str(
            r#"{"sticker_packs":[{"id":"9","name":"Wumpus Beyond","stickers":[{"id":"3","name":"hi","format_type":3}]}]}"#,
        )
        .unwrap();
        assert_eq!(packs.sticker_packs[0].stickers[0].format_type, 3);
    }
}
