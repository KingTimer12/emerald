//! Stickers: on messages, in guilds and in the standard packs.

use serde::Deserialize;

use crate::model::Snowflake;
use crate::urls::{CDN, MEDIA};

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

impl StickerItem {
    /// Where the sticker is downloaded from, or `None` for formats newer
    /// than this client. APNG decodes as its first frame. Lottie files come
    /// only from the CDN: the media proxy does not serve them.
    pub fn url(&self) -> Option<String> {
        let ext = match self.format_type {
            sticker_format::PNG | sticker_format::APNG => "png",
            sticker_format::GIF => "gif",
            sticker_format::LOTTIE => return Some(CDN.join(format_args!("/stickers/{}.json", self.id))),
            _ => return None,
        };
        Some(MEDIA.join(format_args!("/stickers/{}.{ext}?size=160", self.id)))
    }

    pub fn is_lottie(&self) -> bool {
        self.format_type == sticker_format::LOTTIE
    }
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
    fn guild_stickers_and_packs_parse() {
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
