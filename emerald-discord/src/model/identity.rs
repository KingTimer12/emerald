//! How someone presents themselves beyond a name and avatar: the server tag
//! they wear, the font and effect of their display name, and the widgets
//! on their profile. Every field is lenient: a shape Discord adds later
//! leaves the field empty instead of failing the user or profile.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};

use super::Snowflake;
use crate::urls::CDN;

/// `user.primary_guild`: the server tag shown after a name.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct PrimaryGuild {
    #[serde(default)]
    pub identity_guild_id: Option<Snowflake>,
    #[serde(default)]
    pub identity_enabled: Option<bool>,
    #[serde(default)]
    pub tag: Option<String>,
    /// Badge image hash.
    #[serde(default)]
    pub badge: Option<String>,
}

impl PrimaryGuild {
    /// Shown only when turned on and naming both a server and a tag.
    pub fn shown(&self) -> bool {
        self.identity_enabled == Some(true) && self.identity_guild_id.is_some() && self.tag.as_deref().is_some_and(|t| !t.is_empty())
    }

    /// The badge drawn `px` wide, at the size the web client asks for it.
    pub fn badge_url(&self, px: u32) -> Option<String> {
        let (guild, badge) = (self.identity_guild_id?, self.badge.as_deref()?);
        let size = match px {
            0..=16 => 16,
            17..=24 => 24,
            25..=32 => 32,
            _ => 40,
        };
        Some(CDN.join(format_args!("/clan-badges/{guild}/{badge}.png?size={size}")))
    }
}

/// `display_name_styles`: a display name's font, effect and colors. Ids are
/// kept as numbers, known or not.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct NameStyle {
    #[serde(default)]
    pub font_id: u32,
    #[serde(default)]
    pub effect_id: u32,
    /// RGB, as many as the effect uses.
    #[serde(default)]
    pub colors: Vec<u32>,
}

/// A profile widget, kept raw: the Board draws the kinds it knows.
#[derive(Clone, Debug, Deserialize)]
pub struct Widget {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub data: serde_json::Value,
}

impl Widget {
    /// `data.type`: "favorite_games", "played_games", "application"…
    pub fn kind(&self) -> &str {
        self.data.get("type").and_then(|t| t.as_str()).unwrap_or_default()
    }

    /// The games a game widget lists, in order, without repeats.
    pub fn game_ids(&self) -> Vec<Snowflake> {
        let mut ids = Vec::new();
        let games = self.data.get("games").and_then(|g| g.as_array());
        for game in games.into_iter().flatten() {
            let id = game.get("game_id").and_then(|id| serde_json::from_value::<Snowflake>(id.clone()).ok());
            if let Some(id) = id.filter(|id| !ids.contains(id)) {
                ids.push(id);
            }
        }
        ids
    }
}

/// A game from `GET /games`, as the Board shows it.
#[derive(Clone, Debug, Deserialize)]
pub struct Game {
    pub id: Snowflake,
    #[serde(default)]
    pub name: String,
    #[serde(default, deserialize_with = "lenient")]
    pub media: Option<GameMedia>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct GameMedia {
    #[serde(default, deserialize_with = "lenient")]
    pub cover: Option<GameAsset>,
    #[serde(default, deserialize_with = "lenient")]
    pub icon: Option<GameAsset>,
}

/// An image of a game: a CDN hash, or a link for the media proxy.
#[derive(Clone, Debug, Deserialize)]
pub struct GameAsset {
    #[serde(rename = "type")]
    pub kind: String,
    pub value: String,
}

impl Game {
    /// The cover, `px` wide; only hashed covers, which the CDN serves.
    pub fn cover_url(&self, px: u32) -> Option<String> {
        let media = self.media.as_ref()?;
        let asset = media.cover.as_ref().or(media.icon.as_ref()).filter(|a| a.kind == "hash")?;
        Some(CDN.join(format_args!("/app-icons/{}/{}.webp?size={px}&keep_aspect_ratio=true", self.id, asset.value)))
    }
}

/// Games asked for per `GET /games`, as the web client batches them.
pub const GAMES_PER_REQUEST: usize = 20;

/// `ids` in the batches `GET /games` is asked with.
pub fn game_batches(ids: &[Snowflake]) -> std::slice::Chunks<'_, Snowflake> {
    ids.chunks(GAMES_PER_REQUEST)
}

/// A role's icon, `px` wide on screen.
pub fn role_icon_url(role: Snowflake, hash: &str, px: u32) -> String {
    CDN.join(format_args!("/role-icons/{role}/{hash}.webp?size={px}&quality=lossless"))
}

/// A field that is `None` when it does not parse, instead of an error.
pub(crate) fn lenient<'de, D: Deserializer<'de>, T: DeserializeOwned>(d: D) -> Result<Option<T>, D::Error> {
    let value = serde_json::Value::deserialize(d)?;
    Ok(serde_json::from_value(value).ok())
}

/// A list whose items that do not parse are left out.
pub(crate) fn lenient_list<'de, D: Deserializer<'de>, T: DeserializeOwned>(d: D) -> Result<Vec<T>, D::Error> {
    let value = serde_json::Value::deserialize(d)?;
    let items = match value {
        serde_json::Value::Array(items) => items,
        _ => return Ok(Vec::new()),
    };
    Ok(items.into_iter().filter_map(|item| serde_json::from_value(item).ok()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_show_only_when_enabled_and_complete() {
        let tag: PrimaryGuild = serde_json::from_str(
            r#"{"identity_guild_id":"10","identity_enabled":true,"tag":"EMRD","badge":"abc"}"#,
        )
        .unwrap();
        assert!(tag.shown());
        assert_eq!(tag.badge_url(14).unwrap(), "https://cdn.discordapp.com/clan-badges/10/abc.png?size=16");
        let off = PrimaryGuild { identity_enabled: Some(false), ..tag.clone() };
        assert!(!off.shown());
        let empty: PrimaryGuild = serde_json::from_str(r#"{"identity_guild_id":null,"identity_enabled":null,"tag":null,"badge":null}"#).unwrap();
        assert!(!empty.shown());
    }

    #[test]
    fn unknown_style_ids_still_parse() {
        let style: NameStyle = serde_json::from_str(r#"{"font_id":999,"effect_id":42,"colors":[16711680]}"#).unwrap();
        assert_eq!((style.font_id, style.effect_id, style.colors[0]), (999, 42, 0xff0000));
    }

    #[test]
    fn forty_five_games_take_three_requests() {
        let ids: Vec<Snowflake> = (0..45).map(Snowflake).collect();
        let sizes: Vec<usize> = game_batches(&ids).map(<[Snowflake]>::len).collect();
        assert_eq!(sizes, [20, 20, 5]);
    }

    #[test]
    fn games_parse_with_their_cover() {
        let game: Game = serde_json::from_str(
            r#"{"id":"12","name":"Celeste","media":{"cover":{"type":"hash","value":"abc"},"banner":{"type":"url","value":"https://x"}}}"#,
        )
        .unwrap();
        assert_eq!(
            game.cover_url(160).unwrap(),
            "https://cdn.discordapp.com/app-icons/12/abc.webp?size=160&keep_aspect_ratio=true"
        );
    }

    #[test]
    fn widgets_give_their_games_once() {
        let widget: Widget = serde_json::from_str(
            r#"{"id":"1","data":{"type":"favorite_games","games":[{"game_id":"5","tags":[]},{"game_id":"5"},{"game_id":"7"}]}}"#,
        )
        .unwrap();
        assert_eq!(widget.kind(), "favorite_games");
        assert_eq!(widget.game_ids(), [Snowflake(5), Snowflake(7)]);
    }
}
