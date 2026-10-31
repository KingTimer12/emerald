use serde::Deserialize;

use super::snowflake::bits;
use super::{Channel, GuildEmoji, Snowflake, StickerItem, User};
use crate::urls::CDN;

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
    /// Anyone may ping it; otherwise only those allowed to mention everyone.
    #[serde(default)]
    pub mentionable: bool,
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

/// GUILD_MEMBER_UPDATE: a member's nickname or roles changed.
#[derive(Debug, Deserialize)]
pub struct MemberUpdate {
    pub guild_id: Snowflake,
    pub user: User,
    #[serde(default)]
    pub nick: Option<String>,
    #[serde(default)]
    pub roles: Vec<Snowflake>,
}

impl MemberUpdate {
    pub fn into_member(self) -> (Snowflake, Snowflake, Member) {
        let id = self.user.id;
        let member = Member {
            user_id: Some(id),
            user: Some(self.user),
            nick: self.nick,
            roles: self.roles,
        };
        (self.guild_id, id, member)
    }
}

/// GUILD_ROLE_CREATE and GUILD_ROLE_UPDATE.
#[derive(Debug, Deserialize)]
pub struct RoleUpdate {
    pub guild_id: Snowflake,
    pub role: Role,
}

#[derive(Debug, Deserialize)]
pub struct RoleDelete {
    pub guild_id: Snowflake,
    pub role_id: Snowflake,
}

#[derive(Debug, Deserialize)]
pub struct MembersChunk {
    pub guild_id: Snowflake,
    #[serde(default)]
    pub members: Vec<Member>,
    /// Echo of the request's nonce, which tells a search apart from the
    /// author lookups.
    #[serde(default)]
    pub nonce: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GuildProperties {
    pub name: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub owner_id: Option<Snowflake>,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub default_message_notifications: Option<u8>,
    /// The banner's CDN hash, drawn atop the channel list.
    #[serde(default)]
    pub banner: Option<String>,
    #[serde(default)]
    pub premium_subscription_count: Option<u32>,
    /// The server shows its boost goal bar under the banner.
    #[serde(default)]
    pub premium_progress_bar_enabled: bool,
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
    pub emojis: Vec<GuildEmoji>,
    #[serde(default)]
    pub stickers: Vec<StickerItem>,
    #[serde(default)]
    pub features: Vec<String>,
    /// Active threads the user can see, when the gateway sends them.
    #[serde(default)]
    pub threads: Vec<Channel>,
    #[serde(default)]
    pub unavailable: bool,
    /// What members are notified of when they chose nothing: 0 every
    /// message, 1 mentions only.
    #[serde(default)]
    pub default_message_notifications: Option<u8>,
    /// Who is in the guild's voice channels.
    #[serde(default)]
    pub voice_states: Vec<super::VoiceState>,
    #[serde(default)]
    pub banner: Option<String>,
    #[serde(default)]
    pub premium_subscription_count: Option<u32>,
    #[serde(default)]
    pub premium_progress_bar_enabled: bool,
    /// Events scheduled on the server; only how many is read for now.
    #[serde(default)]
    pub guild_scheduled_events: Vec<serde::de::IgnoredAny>,
}

impl Guild {
    pub fn display_name(&self) -> &str {
        self.name
            .as_deref()
            .or(self.properties.as_ref().map(|p| p.name.as_str()))
            .unwrap_or("?")
    }

    /// The largest file a boost level lets members upload here, if any.
    pub fn upload_limit(&self) -> Option<u64> {
        let nested = self.properties.iter().flat_map(|p| &p.features);
        let has = |name: &str| self.features.iter().chain(nested.clone()).any(|f| f == name);
        if has("MAX_FILE_SIZE_100_MB") {
            Some(100 * 1024 * 1024)
        } else if has("MAX_FILE_SIZE_50_MB") {
            Some(50 * 1024 * 1024)
        } else {
            None
        }
    }

    pub fn icon_url(&self) -> Option<String> {
        let hash = self
            .icon
            .as_deref()
            .or(self.properties.as_ref().and_then(|p| p.icon.as_deref()))?;
        Some(CDN.join(format_args!("/icons/{}/{hash}.png?size=96", self.id)))
    }

    /// Every message when the guild says so, else mentions only.
    pub fn default_notify(&self) -> super::Notify {
        let level = self
            .default_message_notifications
            .or(self.properties.as_ref().and_then(|p| p.default_message_notifications));
        if level == Some(0) { super::Notify::All } else { super::Notify::Mentions }
    }

    /// The banner at 360px wide rounded up to a power of two, as the web
    /// client asks for it at 1x. Animated banners are drawn still.
    pub fn banner_url(&self) -> Option<String> {
        let hash = self.banner.as_deref().or(self.properties.as_ref().and_then(|p| p.banner.as_deref()))?;
        Some(CDN.join(format_args!("/banners/{}/{hash}.webp?size=512", self.id)))
    }

    /// How many boosts the server has.
    pub fn boosts(&self) -> u32 {
        self.premium_subscription_count
            .or(self.properties.as_ref().and_then(|p| p.premium_subscription_count))
            .unwrap_or(0)
    }

    /// The server shows its boost goal bar.
    pub fn boost_bar(&self) -> bool {
        self.premium_progress_bar_enabled || self.properties.as_ref().is_some_and(|p| p.premium_progress_bar_enabled)
    }

    /// Whether the server has `feature`, wherever the gateway put the list.
    pub fn has_feature(&self, feature: &str) -> bool {
        self.features.iter().chain(self.properties.iter().flat_map(|p| &p.features)).any(|f| f == feature)
    }

    pub fn owner(&self) -> Option<Snowflake> {
        self.owner_id.or(self.properties.as_ref().and_then(|p| p.owner_id))
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct InviteGuild {
    pub id: Snowflake,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub icon: Option<String>,
}

impl InviteGuild {
    pub fn icon_url(&self) -> Option<String> {
        let hash = self.icon.as_deref()?;
        Some(CDN.join(format_args!("/icons/{}/{hash}.png?size=96", self.id)))
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct InviteChannel {
    pub id: Snowflake,
    #[serde(rename = "type")]
    pub kind: u8,
    #[serde(default)]
    pub name: Option<String>,
}

/// An invite as `GET /invites/{code}` resolves it.
#[derive(Clone, Debug, Deserialize)]
pub struct Invite {
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub guild: Option<InviteGuild>,
    #[serde(default)]
    pub channel: Option<InviteChannel>,
    #[serde(default)]
    pub approximate_member_count: Option<u64>,
    #[serde(default)]
    pub approximate_presence_count: Option<u64>,
    /// ISO time it stops working; `None` never expires.
    #[serde(default)]
    pub expires_at: Option<String>,
}

#[cfg(test)]
mod upload_tests {
    use super::*;

    #[test]
    fn boost_features_raise_the_upload_limit() {
        let guild: Guild = serde_json::from_str(
            r#"{"id":"1","properties":{"name":"x","features":["MAX_FILE_SIZE_50_MB"]}}"#,
        )
        .unwrap();
        assert_eq!(guild.upload_limit(), Some(50 * 1024 * 1024));
        let guild: Guild = serde_json::from_str(r#"{"id":"1","features":["MAX_FILE_SIZE_100_MB"]}"#).unwrap();
        assert_eq!(guild.upload_limit(), Some(100 * 1024 * 1024));
        let guild: Guild = serde_json::from_str(r#"{"id":"1"}"#).unwrap();
        assert_eq!(guild.upload_limit(), None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn guild_reads_banner_and_boost_fields_from_properties() {
        let guild: Guild = serde_json::from_value(json!({
            "id": "1",
            "properties": { "name": "g", "banner": "abc", "premium_tier": 2,
                "premium_subscription_count": 14, "premium_progress_bar_enabled": true },
            "guild_scheduled_events": [{}, {}, {}]
        }))
        .unwrap();
        assert_eq!(guild.banner_url().unwrap(), "https://cdn.discordapp.com/banners/1/abc.webp?size=512");
        assert_eq!(guild.boosts(), 14);
        assert!(guild.boost_bar());
        assert_eq!(guild.guild_scheduled_events.len(), 3);
    }

    #[test]
    fn guild_without_the_new_fields_still_parses() {
        let guild: Guild = serde_json::from_value(json!({ "id": "1", "name": "g" })).unwrap();
        assert!(guild.banner_url().is_none());
        assert!(!guild.boost_bar());
        assert_eq!(guild.boosts(), 0);
    }
}
