//! What the UI changes about the account: the synced settings, the
//! profile and the presence.

use std::sync::Arc;

use crate::model::{CustomStatus, Snowflake, Status};

/// One change to the synced settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SettingsEdit {
    /// The status picked, kept for the other devices too.
    Status(Status),
    /// `None` clears it.
    CustomStatus(Option<CustomStatus>),
    /// Whether members of servers the user joins can DM them. `existing`
    /// applies the choice to the servers already joined too: the ids to
    /// restrict, or none to lift every restriction.
    DmsRestricted {
        restricted: bool,
        existing: Option<Vec<Snowflake>>,
    },
    /// Message requests from server members the user may not know, with
    /// `existing` as for DMs.
    MessageRequestsRestricted {
        restricted: bool,
        existing: Option<Vec<Snowflake>>,
    },
    /// Who may send friend requests, as `friend_source` flags.
    FriendSources(u32),
}

/// An image picked from disk: its media type and bytes.
pub type Picture = (&'static str, Arc<[u8]>);

/// Changes to the profile, global or for one server. A field left `None`
/// stays as it is; `Some(None)` (or an empty text) clears it.
#[derive(Clone, Debug, Default)]
pub struct ProfileEdit {
    /// The display name, or the nickname in a server.
    pub name: Option<Option<String>>,
    pub avatar: Option<Option<Picture>>,
    pub banner: Option<Option<Picture>>,
    pub bio: Option<String>,
    pub pronouns: Option<String>,
    /// The banner's color when there is no banner image; global only.
    pub accent_color: Option<Option<u32>>,
}

impl ProfileEdit {
    /// Fields saved on the user, or on the server membership.
    pub fn touches_user(&self) -> bool {
        self.name.is_some() || self.avatar.is_some()
    }

    /// Fields saved on the profile.
    pub fn touches_profile(&self) -> bool {
        self.banner.is_some() || self.bio.is_some() || self.pronouns.is_some() || self.accent_color.is_some()
    }
}

/// What the user shows the others: the status picked and the custom
/// status, sent over the gateway as presence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnPresence {
    pub status: Status,
    pub custom: Option<CustomStatus>,
}
