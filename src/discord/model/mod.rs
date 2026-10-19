//! Minimal Discord payload shapes. Only fields the UI renders are kept;
//! serde skips everything else, so large payloads (READY, GUILD_CREATE)
//! never materialize members, presences, roles or emojis in memory.

use std::fmt;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Snowflake(pub u64);

/// Discord epoch (2015-01-01T00:00:00Z) in Unix milliseconds.
pub const DISCORD_EPOCH_MS: u64 = 1_420_070_400_000;

impl Snowflake {
    /// Creation time encoded in the id, in Unix seconds.
    pub fn unix_secs(self) -> i64 {
        (((self.0 >> 22) + DISCORD_EPOCH_MS) / 1000) as i64
    }
}

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
    pub const VOICE: u8 = 2;
    pub const GROUP_DM: u8 = 3;
    pub const CATEGORY: u8 = 4;
    pub const ANNOUNCEMENT: u8 = 5;
    pub const STAGE: u8 = 13;
    pub const FORUM: u8 = 15;
    pub const MEDIA: u8 = 16;
}

/// Permission bitfields arrive as decimal strings.
fn bits<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    struct V;
    impl Visitor<'_> for V {
        type Value = u64;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a permission bitfield")
        }
        fn visit_str<E: de::Error>(self, v: &str) -> Result<u64, E> {
            v.parse().map_err(E::custom)
        }
        fn visit_u64<E: de::Error>(self, v: u64) -> Result<u64, E> {
            Ok(v)
        }
    }
    d.deserialize_any(V)
}

#[derive(Clone, Debug, Deserialize)]
pub struct Role {
    pub id: Snowflake,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub color: u32,
    #[serde(default)]
    pub position: i32,
    #[serde(default, deserialize_with = "bits")]
    pub permissions: u64,
}

pub mod overwrite_kind {
    pub const ROLE: u8 = 0;
    pub const MEMBER: u8 = 1;
}

#[derive(Clone, Debug, Deserialize)]
pub struct Overwrite {
    pub id: Snowflake,
    #[serde(rename = "type")]
    pub kind: u8,
    #[serde(default, deserialize_with = "bits")]
    pub allow: u64,
    #[serde(default, deserialize_with = "bits")]
    pub deny: u64,
}

/// The current user's (or a message author's) membership in a guild.
#[derive(Clone, Debug, Deserialize)]
pub struct Member {
    /// READY's `merged_members` carries `user_id`; chunks carry `user`.
    #[serde(default)]
    pub user_id: Option<Snowflake>,
    #[serde(default)]
    pub user: Option<User>,
    #[serde(default)]
    pub nick: Option<String>,
    #[serde(default)]
    pub roles: Vec<Snowflake>,
}

impl Member {
    pub fn id(&self) -> Option<Snowflake> {
        self.user_id.or(self.user.as_ref().map(|u| u.id))
    }
}

#[derive(Debug, Deserialize)]
pub struct MembersChunk {
    pub guild_id: Snowflake,
    #[serde(default)]
    pub members: Vec<Member>,
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
    #[serde(default)]
    pub permission_overwrites: Vec<Overwrite>,
    #[serde(default)]
    pub topic: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GuildProperties {
    pub name: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub owner_id: Option<Snowflake>,
}

#[derive(Debug, Deserialize)]
pub struct Guild {
    pub id: Snowflake,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub owner_id: Option<Snowflake>,
    /// Newer user-gateway READY shapes nest name, icon and owner here.
    #[serde(default)]
    pub properties: Option<GuildProperties>,
    #[serde(default)]
    pub channels: Vec<Channel>,
    #[serde(default)]
    pub roles: Vec<Role>,
    /// GUILD_CREATE may list the current member here.
    #[serde(default)]
    pub members: Vec<Member>,
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

    pub fn icon_url(&self) -> Option<String> {
        let hash = self
            .icon
            .as_deref()
            .or(self.properties.as_ref().and_then(|p| p.icon.as_deref()))?;
        Some(format!("https://cdn.discordapp.com/icons/{}/{hash}.png?size=96", self.id))
    }

    pub fn owner(&self) -> Option<Snowflake> {
        self.owner_id.or(self.properties.as_ref().and_then(|p| p.owner_id))
    }
}

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
    pub const GIF: u8 = 4;
}

#[derive(Debug, Deserialize)]
pub struct StickerItem {
    pub id: Snowflake,
    pub name: String,
    pub format_type: u8,
}

impl StickerItem {
    /// Image URL, or `None` for Lottie stickers, which are vector
    /// animations this client cannot draw. APNG decodes as its first frame.
    pub fn url(&self) -> Option<String> {
        let ext = match self.format_type {
            sticker_format::PNG | sticker_format::APNG => "png",
            sticker_format::GIF => "gif",
            // Lottie (3) and formats newer than this client.
            _ => return None,
        };
        Some(format!("https://media.discordapp.net/stickers/{}.{ext}?size=160", self.id))
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
    pub attachments: Vec<Attachment>,
    #[serde(default)]
    pub embeds: Vec<Embed>,
    /// The message this one replies to; `None` inside or when deleted.
    #[serde(default)]
    pub referenced_message: Option<Box<Message>>,
    #[serde(default)]
    pub sticker_items: Vec<StickerItem>,
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

#[derive(Debug, Deserialize)]
pub struct MessageUpdate {
    pub id: Snowflake,
    pub channel_id: Snowflake,
    #[serde(default)]
    pub content: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TypingStart {
    pub channel_id: Snowflake,
    pub user_id: Snowflake,
}

#[derive(Debug, Default, Deserialize)]
pub struct UserProfileDetails {
    #[serde(default)]
    pub bio: Option<String>,
    #[serde(default)]
    pub pronouns: Option<String>,
}

/// `GET /users/{id}/profile`, as the DM profile panel uses it.
#[derive(Debug, Deserialize)]
pub struct Profile {
    pub user: ProfileUser,
    #[serde(default)]
    pub user_profile: UserProfileDetails,
    #[serde(default)]
    pub badges: Vec<Badge>,
    #[serde(default)]
    pub connected_accounts: Vec<ConnectedAccount>,
}

#[derive(Debug, Deserialize)]
pub struct Badge {
    /// Hover text, already localized.
    pub description: String,
    /// Icon hash under `badge-icons/`.
    pub icon: String,
}

impl Badge {
    pub fn icon_url(&self) -> String {
        format!("https://cdn.discordapp.com/badge-icons/{}.png", self.icon)
    }
}

#[derive(Debug, Deserialize)]
pub struct ConnectedAccount {
    /// Service, e.g. `github` or `steam`.
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct ProfileUser {
    pub id: Snowflake,
    /// Banner fill when there is no banner image, as `#rrggbb`.
    #[serde(default)]
    pub banner_color: Option<String>,
    #[serde(default)]
    pub accent_color: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct MessageAck {
    pub channel_id: Snowflake,
    pub message_id: Snowflake,
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
    /// The current user's member object per guild, in `guilds` order.
    #[serde(default)]
    pub merged_members: Vec<Vec<Member>>,
    #[serde(default)]
    pub read_state: ReadStates,
    #[serde(default)]
    pub auth_token: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ReadStates {
    #[serde(default)]
    pub entries: Vec<ReadState>,
}

/// Last message the user has read in a channel.
#[derive(Debug, Deserialize)]
pub struct ReadState {
    pub id: Snowflake,
    #[serde(default)]
    pub last_message_id: Option<Snowflake>,
    /// Non-channel read states (e.g. guild events) use other types.
    #[serde(default)]
    pub read_state_type: u8,
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
    fn sticker_urls_skip_lottie() {
        let sticker = |format_type| StickerItem {
            id: Snowflake(7),
            name: "wave".into(),
            format_type,
        };
        assert_eq!(
            sticker(sticker_format::GIF).url().as_deref(),
            Some("https://media.discordapp.net/stickers/7.gif?size=160")
        );
        assert_eq!(sticker(3).url(), None);
    }

    #[test]
    fn snowflake_time() {
        // Discord's docs example: 175928847299117063 is 2016-04-30T11:18:25.796Z.
        assert_eq!(Snowflake(175928847299117063).unix_secs(), 1462015105);
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
