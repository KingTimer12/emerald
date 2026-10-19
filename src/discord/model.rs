//! Minimal Discord payload shapes. Only fields the UI renders are kept;
//! serde skips everything else, so large payloads (READY, GUILD_CREATE)
//! never materialize members, presences, roles or emojis in memory.

use std::fmt;

use serde::de::{self, IgnoredAny, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Snowflake(pub u64);

impl fmt::Display for Snowflake {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<'de> Deserialize<'de> for Snowflake {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl Visitor<'_> for V {
            type Value = Snowflake;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a snowflake id")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Snowflake, E> {
                v.parse().map(Snowflake).map_err(E::custom)
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Snowflake, E> {
                Ok(Snowflake(v))
            }
        }
        d.deserialize_any(V)
    }
}

pub mod channel_kind {
    pub const GUILD_TEXT: u8 = 0;
    pub const DM: u8 = 1;
    pub const GROUP_DM: u8 = 3;
    pub const CATEGORY: u8 = 4;
    pub const ANNOUNCEMENT: u8 = 5;
}

#[derive(Clone, Debug, Deserialize)]
pub struct User {
    pub id: Snowflake,
    pub username: String,
    #[serde(default)]
    pub global_name: Option<String>,
    /// Avatar hash; `None` means Discord's default avatar.
    #[serde(default)]
    pub avatar: Option<String>,
    /// Only present on the current user (READY, USER_UPDATE).
    #[serde(default)]
    pub email: Option<String>,
}

impl User {
    pub fn display_name(&self) -> &str {
        self.global_name.as_deref().unwrap_or(&self.username)
    }

    pub fn avatar_url(&self) -> String {
        avatar_url(self.id, self.avatar.as_deref())
    }
}

/// CDN URL of a user's avatar, as a PNG sized for 2x displays. Without a
/// hash Discord shows one of six default avatars, picked by `(id >> 22) % 6`
/// for accounts on the current username system.
pub fn avatar_url(id: Snowflake, hash: Option<&str>) -> String {
    match hash {
        Some(hash) => format!("https://cdn.discordapp.com/avatars/{id}/{hash}.png?size=128"),
        None => format!("https://cdn.discordapp.com/embed/avatars/{}.png", (id.0 >> 22) % 6),
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Channel {
    pub id: Snowflake,
    #[serde(rename = "type")]
    pub kind: u8,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub position: i32,
    #[serde(default)]
    pub parent_id: Option<Snowflake>,
    #[serde(default)]
    pub guild_id: Option<Snowflake>,
    #[serde(default)]
    pub last_message_id: Option<Snowflake>,
    #[serde(default)]
    pub recipient_ids: Vec<Snowflake>,
    #[serde(default)]
    pub recipients: Vec<User>,
}

#[derive(Debug, Deserialize)]
pub struct GuildProperties {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct Guild {
    pub id: Snowflake,
    #[serde(default)]
    pub name: Option<String>,
    /// Newer user-gateway READY shapes nest the name here.
    #[serde(default)]
    pub properties: Option<GuildProperties>,
    #[serde(default)]
    pub channels: Vec<Channel>,
    #[serde(default)]
    pub unavailable: bool,
}

impl Guild {
    pub fn display_name(&self) -> &str {
        self.name
            .as_deref()
            .or(self.properties.as_ref().map(|p| p.name.as_str()))
            .unwrap_or("?")
    }
}

#[derive(Debug, Deserialize)]
pub struct Attachment {
    pub filename: String,
}

#[derive(Debug, Deserialize)]
pub struct Message {
    pub id: Snowflake,
    pub channel_id: Snowflake,
    pub author: User,
    #[serde(default)]
    pub content: String,
    pub timestamp: String,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    #[serde(default)]
    pub embeds: Vec<IgnoredAny>,
}

#[derive(Debug, Deserialize)]
pub struct MessageUpdate {
    pub id: Snowflake,
    pub channel_id: Snowflake,
    #[serde(default)]
    pub content: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MessageDelete {
    pub id: Snowflake,
    pub channel_id: Snowflake,
}

#[derive(Debug, Deserialize)]
pub struct Ready {
    pub session_id: String,
    pub resume_gateway_url: String,
    pub user: User,
    #[serde(default)]
    pub guilds: Vec<Guild>,
    #[serde(default)]
    pub private_channels: Vec<Channel>,
    #[serde(default)]
    pub users: Vec<User>,
    #[serde(default)]
    pub relationships: Vec<Relationship>,
    #[serde(default)]
    pub auth_token: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ReadySupplemental {
    #[serde(default)]
    pub merged_presences: MergedPresences,
}

#[derive(Debug, Default, Deserialize)]
pub struct MergedPresences {
    #[serde(default)]
    pub friends: Vec<Presence>,
}

pub mod relationship_kind {
    pub const FRIEND: u8 = 1;
    pub const INCOMING: u8 = 3;
    pub const OUTGOING: u8 = 4;
}

/// A relationship to another user. With deduplicated user objects READY
/// carries only `user_id` and lists the user in `users`; RELATIONSHIP_ADD
/// carries the full `user`.
#[derive(Clone, Debug, Deserialize)]
pub struct Relationship {
    /// The other user's id.
    pub id: Snowflake,
    #[serde(rename = "type")]
    pub kind: u8,
    #[serde(default)]
    pub user: Option<User>,
}

#[derive(Debug, Deserialize)]
pub struct RelationshipRemove {
    pub id: Snowflake,
}

#[derive(Debug, Deserialize)]
pub struct PresenceUser {
    pub id: Snowflake,
}

/// Friend presence: `user_id` in READY_SUPPLEMENTAL, `user.id` in
/// PRESENCE_UPDATE. Guild presences carry `guild_id` and are ignored.
#[derive(Debug, Deserialize)]
pub struct Presence {
    #[serde(default)]
    pub user_id: Option<Snowflake>,
    #[serde(default)]
    pub user: Option<PresenceUser>,
    pub status: String,
    #[serde(default)]
    pub guild_id: Option<Snowflake>,
}

impl Presence {
    pub fn user(&self) -> Option<Snowflake> {
        self.user_id.or(self.user.as_ref().map(|u| u.id))
    }
}

#[derive(Debug, Deserialize)]
pub struct InviteGuild {
    pub id: Snowflake,
}

#[derive(Debug, Deserialize)]
pub struct InviteChannel {
    pub id: Snowflake,
    #[serde(rename = "type")]
    pub kind: u8,
}

#[derive(Debug, Deserialize)]
pub struct Invite {
    #[serde(default)]
    pub guild: Option<InviteGuild>,
    #[serde(default)]
    pub channel: Option<InviteChannel>,
}

/// Presence the user picks; sent in IDENTIFY and in presence updates (op 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Online,
    Idle,
    Dnd,
    Invisible,
}

impl Status {
    pub const ALL: [Status; 4] = [Status::Online, Status::Idle, Status::Dnd, Status::Invisible];

    pub fn as_str(self) -> &'static str {
        match self {
            Status::Online => "online",
            Status::Idle => "idle",
            Status::Dnd => "dnd",
            Status::Invisible => "invisible",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avatar_url_uses_hash_or_default_index() {
        let id = Snowflake(80351110224678912);
        assert_eq!(
            avatar_url(id, Some("abc")),
            "https://cdn.discordapp.com/avatars/80351110224678912/abc.png?size=128"
        );
        assert_eq!(
            avatar_url(id, None),
            format!("https://cdn.discordapp.com/embed/avatars/{}.png", (id.0 >> 22) % 6)
        );
    }
}
