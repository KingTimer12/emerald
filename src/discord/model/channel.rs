use serde::Deserialize;

use super::{Overwrite, Snowflake, User};

/// Discord's channel `type`, with what this client does with each kind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Deserialize)]
#[serde(transparent)]
pub struct ChannelKind(pub u8);

impl ChannelKind {
    pub const GUILD_TEXT: Self = Self(0);
    pub const DM: Self = Self(1);
    pub const VOICE: Self = Self(2);
    pub const GROUP_DM: Self = Self(3);
    pub const CATEGORY: Self = Self(4);
    pub const ANNOUNCEMENT: Self = Self(5);
    pub const STAGE: Self = Self(13);
    pub const FORUM: Self = Self(15);
    pub const MEDIA: Self = Self(16);

    /// Text channels the chat can open; voice, stage and forum are listed only.
    pub fn is_openable(self) -> bool {
        matches!(self, Self::GUILD_TEXT | Self::ANNOUNCEMENT | Self::DM | Self::GROUP_DM)
    }

    pub fn is_private(self) -> bool {
        matches!(self, Self::DM | Self::GROUP_DM)
    }

    /// Voice-like channels, which Discord lists after the text-like ones.
    pub fn is_voice(self) -> bool {
        matches!(self, Self::VOICE | Self::STAGE)
    }

    /// Guild channels shown in the sidebar; threads and unknown kinds are not.
    pub fn is_listed(self) -> bool {
        matches!(
            self,
            Self::GUILD_TEXT | Self::VOICE | Self::ANNOUNCEMENT | Self::STAGE | Self::FORUM | Self::MEDIA
        )
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Channel {
    pub id: Snowflake,
    #[serde(rename = "type")]
    pub kind: ChannelKind,
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

impl Channel {
    /// The other person in a 1:1 DM.
    pub fn recipient(&self) -> Option<Snowflake> {
        if self.kind != ChannelKind::DM {
            return None;
        }
        self.recipient_ids
            .first()
            .copied()
            .or(self.recipients.first().map(|u| u.id))
    }

    /// Where the activity sort puts it: its newest message, else when it
    /// was created, which its id encodes.
    pub fn activity(&self) -> Snowflake {
        self.last_message_id.unwrap_or(self.id)
    }
}
