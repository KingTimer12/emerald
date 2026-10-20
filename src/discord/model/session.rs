//! Payloads that open a gateway session.

use serde::Deserialize;

use super::{Channel, Guild, Member, Presence, Relationship, Snowflake, User};

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
    /// Unread mentions; every unread message counts in a DM.
    #[serde(default)]
    pub mention_count: u32,
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
