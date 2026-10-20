use serde::{Deserialize, Serialize};

use super::Snowflake;

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
    /// Bots and apps, tagged "APP" next to their name.
    #[serde(default)]
    pub bot: bool,
    /// Nitro tier, 0 for none. Only present on the current user.
    #[serde(default)]
    pub premium_type: Option<u8>,
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
