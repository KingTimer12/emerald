//! Emoji as Discord sends them in reactions and guild lists, and the
//! reaction gateway events.

use serde::Deserialize;

use super::Snowflake;

/// A Unicode emoji (`id` unset, `name` the emoji itself) or a custom one.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Hash)]
pub struct Emoji {
    #[serde(default)]
    pub id: Option<Snowflake>,
    /// `None` for a custom emoji that was deleted.
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub animated: bool,
}

impl Emoji {
    pub fn unicode(emoji: &str) -> Self {
        Self {
            id: None,
            name: Some(emoji.to_owned()),
            animated: false,
        }
    }

    /// Discord compares custom emoji by id and Unicode ones by text.
    pub fn same(&self, other: &Self) -> bool {
        match (self.id, other.id) {
            (Some(a), Some(b)) => a == b,
            (None, None) => self.name == other.name,
            _ => false,
        }
    }

    /// The emoji as reaction routes take it: `name:id`, or the text itself.
    pub fn route_key(&self) -> String {
        let name = self.name.as_deref().unwrap_or("_");
        match self.id {
            Some(id) => format!("{name}:{id}"),
            None => name.to_owned(),
        }
    }
}

/// CDN image of a custom emoji, sized for 2x displays. Animated emoji come
/// as their first frame.
pub fn custom_url(id: Snowflake) -> String {
    format!("https://cdn.discordapp.com/emojis/{id}.png?size=48")
}

/// An emoji uploaded to a guild.
#[derive(Clone, Debug, Deserialize)]
pub struct GuildEmoji {
    pub id: Snowflake,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub animated: bool,
    /// False once the guild lost the boost level that unlocked it.
    #[serde(default = "yes")]
    pub available: bool,
}

fn yes() -> bool {
    true
}

impl GuildEmoji {
    pub fn emoji(&self) -> Emoji {
        Emoji {
            id: Some(self.id),
            name: Some(self.name.clone()),
            animated: self.animated,
        }
    }
}

/// GUILD_EMOJIS_UPDATE: the guild's whole list.
#[derive(Debug, Deserialize)]
pub struct GuildEmojisUpdate {
    pub guild_id: Snowflake,
    #[serde(default)]
    pub emojis: Vec<GuildEmoji>,
}

/// A message's reaction with one emoji. Super reactions ("burst") count
/// apart from normal ones.
#[derive(Clone, Debug, Deserialize)]
pub struct Reaction {
    pub emoji: Emoji,
    #[serde(default)]
    pub count: u32,
    #[serde(default)]
    pub count_details: Option<CountDetails>,
    #[serde(default)]
    pub me: bool,
    #[serde(default)]
    pub me_burst: bool,
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
pub struct CountDetails {
    #[serde(default)]
    pub normal: u32,
    #[serde(default)]
    pub burst: u32,
}

impl Reaction {
    /// Normal and super reaction counts; older payloads have only `count`.
    pub fn counts(&self) -> CountDetails {
        self.count_details.unwrap_or(CountDetails {
            normal: self.count,
            burst: 0,
        })
    }
}

/// MESSAGE_REACTION_ADD and MESSAGE_REACTION_REMOVE.
#[derive(Debug, Deserialize)]
pub struct ReactionUpdate {
    pub user_id: Snowflake,
    pub channel_id: Snowflake,
    pub message_id: Snowflake,
    pub emoji: Emoji,
    #[serde(default)]
    pub burst: bool,
}

/// MESSAGE_REACTION_ADD_MANY: several users' reactions batched together.
#[derive(Debug, Deserialize)]
pub struct ReactionAddMany {
    pub channel_id: Snowflake,
    pub message_id: Snowflake,
    #[serde(default)]
    pub reactions: Vec<ReactionBatch>,
}

#[derive(Debug, Deserialize)]
pub struct ReactionBatch {
    pub emoji: Emoji,
    #[serde(default)]
    pub users: Vec<Snowflake>,
    #[serde(default)]
    pub burst: bool,
}

/// MESSAGE_REACTION_REMOVE_ALL, and MESSAGE_REACTION_REMOVE_EMOJI with `emoji`.
#[derive(Debug, Deserialize)]
pub struct ReactionClear {
    pub channel_id: Snowflake,
    pub message_id: Snowflake,
    #[serde(default)]
    pub emoji: Option<Emoji>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_keys_match_the_web_client() {
        assert_eq!(Emoji::unicode("👍").route_key(), "👍");
        let custom = Emoji {
            id: Some(Snowflake(42)),
            name: Some("pepe".into()),
            animated: false,
        };
        assert_eq!(custom.route_key(), "pepe:42");
    }

    #[test]
    fn custom_emoji_compare_by_id() {
        let a = Emoji {
            id: Some(Snowflake(1)),
            name: Some("a".into()),
            animated: false,
        };
        let renamed = Emoji {
            name: Some("b".into()),
            ..a.clone()
        };
        assert!(a.same(&renamed));
        assert!(!a.same(&Emoji::unicode("a")));
        assert!(Emoji::unicode("👍").same(&Emoji::unicode("👍")));
    }

    #[test]
    fn counts_fall_back_to_count() {
        let reaction: Reaction =
            serde_json::from_str(r#"{"emoji":{"id":null,"name":"👍"},"count":3,"me":true}"#).unwrap();
        assert_eq!(reaction.counts().normal, 3);
        let reaction: Reaction = serde_json::from_str(
            r#"{"emoji":{"id":null,"name":"👍"},"count":3,"count_details":{"normal":2,"burst":1},"me":false,"me_burst":true,"burst_colors":[]}"#,
        )
        .unwrap();
        assert_eq!((reaction.counts().normal, reaction.counts().burst), (2, 1));
    }
}
