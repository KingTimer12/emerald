use serde::Deserialize;

use super::{Overwrite, Snowflake, User};
use crate::urls::{CDN, WEB};

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
    pub const ANNOUNCEMENT_THREAD: Self = Self(10);
    pub const PUBLIC_THREAD: Self = Self(11);
    pub const PRIVATE_THREAD: Self = Self(12);
    pub const STAGE: Self = Self(13);
    pub const FORUM: Self = Self(15);
    pub const MEDIA: Self = Self(16);

    /// Channels the chat can open: text ones and threads show messages,
    /// forums their posts; voice and stage are listed only.
    pub fn is_openable(self) -> bool {
        self.is_thread()
            || self.is_forum()
            || matches!(self, Self::GUILD_TEXT | Self::ANNOUNCEMENT | Self::DM | Self::GROUP_DM)
    }

    pub fn is_thread(self) -> bool {
        matches!(self, Self::ANNOUNCEMENT_THREAD | Self::PUBLIC_THREAD | Self::PRIVATE_THREAD)
    }

    /// Forum and media channels hold only posts, each a thread.
    pub fn is_forum(self) -> bool {
        matches!(self, Self::FORUM | Self::MEDIA)
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
    /// Threads: their state, and ours in them when we joined.
    #[serde(default)]
    pub thread_metadata: Option<ThreadMetadata>,
    #[serde(default)]
    pub member: Option<ThreadMember>,
    #[serde(default)]
    pub message_count: Option<u32>,
    /// Forum posts: the tags given, from the forum's `available_tags`.
    #[serde(default)]
    pub applied_tags: Vec<Snowflake>,
    #[serde(default)]
    pub available_tags: Vec<ForumTag>,
    /// Forums: 0 lists by latest activity, 1 by creation.
    #[serde(default)]
    pub default_sort_order: Option<u8>,
    /// Minutes of quiet before a new thread here archives itself.
    #[serde(default)]
    pub default_auto_archive_duration: Option<u32>,
    #[serde(default)]
    pub flags: u64,
    /// Group DMs: the icon hash, and who may remove people.
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub owner_id: Option<Snowflake>,
    /// A DM from someone the user has not accepted yet; with `is_spam`
    /// it sits in the spam inbox instead.
    #[serde(default)]
    pub is_message_request: bool,
    #[serde(default)]
    pub is_spam: bool,
}

/// The web client's eight group DM pictures, for groups without an icon;
/// one is picked by the channel's creation time.
const DEFAULT_GROUP_ICONS: [&str; 8] = [
    "b8c21efe7946ee22",
    "392f18c1d7fa4cf1",
    "f06f874912a9a404",
    "84ada88a442f014d",
    "adc958faf2495e83",
    "e8a3e602806eb4e0",
    "2ccd0ae2e8934844",
    "95fad9fee060f9ae",
];

/// Channel flag on forums that refuse untagged posts.
const REQUIRE_TAG: u64 = 1 << 4;
/// Archive delay of new threads when the parent sets none, in minutes.
pub const DEFAULT_AUTO_ARCHIVE: u32 = 3 * 24 * 60;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ThreadMetadata {
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub locked: bool,
}

/// The current user's membership of a thread.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ThreadMember {
    #[serde(default)]
    pub join_timestamp: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ForumTag {
    pub id: Snowflake,
    #[serde(default)]
    pub name: String,
    /// Unicode emoji only; custom ones are not drawn on tags.
    #[serde(default)]
    pub emoji_name: Option<String>,
    /// Only those who manage threads may apply it.
    #[serde(default)]
    pub moderated: bool,
}

/// THREAD_LIST_SYNC: active threads of a guild, after op 37 asks for them.
#[derive(Debug, Deserialize)]
pub struct ThreadListSync {
    pub guild_id: Snowflake,
    #[serde(default)]
    pub threads: Vec<Channel>,
    /// Our memberships among `threads`.
    #[serde(default)]
    pub members: Vec<ThreadMemberEntry>,
}

/// THREAD_MEMBER_UPDATE, and the entries of THREAD_LIST_SYNC and
/// THREAD_MEMBERS_UPDATE.
#[derive(Debug, Deserialize)]
pub struct ThreadMemberEntry {
    /// The thread.
    pub id: Snowflake,
    #[serde(default)]
    pub guild_id: Option<Snowflake>,
    #[serde(default)]
    pub user_id: Option<Snowflake>,
    #[serde(default)]
    pub join_timestamp: Option<String>,
}

impl ThreadMemberEntry {
    pub fn member(&self) -> ThreadMember {
        ThreadMember {
            join_timestamp: self.join_timestamp.clone(),
        }
    }
}

/// THREAD_MEMBERS_UPDATE: who joined or left a thread.
#[derive(Debug, Deserialize)]
pub struct ThreadMembersUpdate {
    pub id: Snowflake,
    pub guild_id: Snowflake,
    #[serde(default)]
    pub added_members: Vec<ThreadMemberEntry>,
    #[serde(default)]
    pub removed_member_ids: Vec<Snowflake>,
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

    /// A group DM's picture: its icon, or the default its id picks.
    pub fn group_icon_url(&self) -> String {
        match &self.icon {
            Some(hash) => CDN.join(format_args!("/channel-icons/{}/{hash}.png?size=64", self.id)),
            None => {
                let created_ms = (self.id.0 >> 22) + super::DISCORD_EPOCH_MS;
                let ix = (created_ms % DEFAULT_GROUP_ICONS.len() as u64) as usize;
                WEB.join(format_args!("/assets/{}.png", DEFAULT_GROUP_ICONS[ix]))
            }
        }
    }

    /// Waiting in "Message Requests", outside the DM list.
    pub fn is_request(&self) -> bool {
        self.is_message_request && !self.is_spam
    }

    /// Waiting in the spam inbox.
    pub fn is_spam_request(&self) -> bool {
        self.is_message_request && self.is_spam
    }

    /// A thread that is not archived; archived ones leave the sidebar.
    pub fn is_active_thread(&self) -> bool {
        self.kind.is_thread() && !self.thread_metadata.as_ref().is_some_and(|m| m.archived)
    }

    /// A forum whose posts must carry a tag.
    pub fn requires_tag(&self) -> bool {
        self.flags & REQUIRE_TAG != 0
    }

    pub fn is_locked(&self) -> bool {
        self.thread_metadata.as_ref().is_some_and(|m| m.locked)
    }

    /// Where the activity sort puts it: its newest message, else when it
    /// was created, which its id encodes.
    pub fn activity(&self) -> Snowflake {
        self.last_message_id.unwrap_or(self.id)
    }
}

/// CHANNEL_RECIPIENT_ADD and CHANNEL_RECIPIENT_REMOVE: someone joined or
/// left a group DM.
#[derive(Debug, Deserialize)]
pub struct RecipientUpdate {
    pub channel_id: Snowflake,
    pub user: User,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_without_icon_get_a_default_picture() {
        let group = |icon: Option<&str>| Channel {
            id: Snowflake(5 << 22),
            kind: ChannelKind::GROUP_DM,
            icon: icon.map(Into::into),
            ..Channel::default()
        };
        assert_eq!(group(Some("h")).group_icon_url(), format!("https://cdn.discordapp.com/channel-icons/{}/h.png?size=64", 5u64 << 22));
        let index = (5 + super::super::DISCORD_EPOCH_MS) % 8;
        assert_eq!(group(None).group_icon_url(), format!("https://discord.com/assets/{}.png", DEFAULT_GROUP_ICONS[index as usize]));
    }
}
