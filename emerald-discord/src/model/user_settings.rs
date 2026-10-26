//! The account's synced settings that Emerald reads and writes: the custom
//! status and the privacy choices, kept inside `PreloadedUserSettings`.
//!
//! Discord saves a settings PATCH a whole top-level section at a time, so
//! the raw bytes of each section are kept: an edit rewrites only its own
//! fields of the section and sends the rest back as they were.

use super::{Snowflake, Status};
use super::proto::{self, Reader, Value, wrapped_bytes, wrapped_varint};

/// `PreloadedUserSettings` field numbers, from the web client's protobuf
/// definitions.
pub(crate) mod field {
    pub const VERSIONS: u32 = 1;
    pub const PRIVACY: u32 = 8;
    pub const STATUS: u32 = 11;

    /// `Versions`.
    pub const DATA_VERSION: u32 = 3;

    /// `StatusSettings`.
    pub const STATUS_NAME: u32 = 1;
    pub const CUSTOM_STATUS: u32 = 2;
    /// `CustomStatus`.
    pub const TEXT: u32 = 1;
    pub const EMOJI_ID: u32 = 2;
    pub const EMOJI_NAME: u32 = 3;
    pub const EXPIRES_AT_MS: u32 = 4;
    pub const CREATED_AT_MS: u32 = 5;

    /// `PrivacySettings`.
    pub const RESTRICTED_GUILD_IDS: u32 = 3;
    pub const DEFAULT_GUILDS_RESTRICTED: u32 = 4;
    pub const FRIEND_SOURCE_FLAGS: u32 = 11;
    pub const MESSAGE_REQUEST_RESTRICTED_GUILD_IDS: u32 = 16;
    pub const DEFAULT_MESSAGE_REQUEST_RESTRICTED: u32 = 17;
    pub const DEFAULT_GUILDS_RESTRICTED_V2: u32 = 27;
}

/// Who may send the user a friend request, as `friend_source_flags`.
pub mod friend_source {
    pub const MUTUAL_FRIENDS: u32 = 2;
    pub const MUTUAL_GUILDS: u32 = 4;
    pub const NO_RELATION: u32 = 8;
    /// The default: all three.
    pub const EVERYONE: u32 = MUTUAL_FRIENDS | MUTUAL_GUILDS | NO_RELATION;
}

/// The sections Emerald reads, as they came. `None` for a section the
/// proto left out.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UserSettings {
    pub(crate) versions: Option<Vec<u8>>,
    pub(crate) privacy: Option<Vec<u8>>,
    pub(crate) status: Option<Vec<u8>>,
}

/// The text and emoji under the user's name, and when it clears itself.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CustomStatus {
    pub text: String,
    /// A custom emoji; `emoji_name` then holds its name.
    pub emoji_id: Option<Snowflake>,
    /// A Unicode emoji, or the name of the custom one.
    pub emoji_name: Option<String>,
    /// Unix milliseconds; 0 keeps it until cleared.
    pub expires_at_ms: u64,
}

impl CustomStatus {
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty() && self.emoji_name.is_none()
    }

    pub fn expired(&self, now_ms: u64) -> bool {
        self.expires_at_ms != 0 && self.expires_at_ms <= now_ms
    }

    /// As others read it: a Unicode emoji, then the text.
    pub fn line(&self) -> String {
        match self.emoji_name.as_deref().filter(|_| self.emoji_id.is_none()) {
            Some(emoji) if self.text.is_empty() => emoji.to_owned(),
            Some(emoji) => format!("{emoji} {}", self.text),
            None => self.text.clone(),
        }
    }
}

/// The privacy choices the Privacy & Safety page shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Privacy {
    /// DMs from members of servers the user joins are off by default.
    pub dms_restricted: bool,
    /// Message requests from server members the user may not know are off
    /// by default.
    pub message_requests_restricted: bool,
    /// [`friend_source`] flags.
    pub friend_sources: u32,
}

impl UserSettings {
    /// The sections of `proto`, base64 as READY and USER_SETTINGS_PROTO_UPDATE
    /// send it; `None` when it is malformed.
    pub fn decode(proto: &str) -> Option<Self> {
        let bytes = proto::decode_base64(proto)?;
        let mut settings = Self::default();
        for (number, value) in Reader::new(&bytes).fields().ok()? {
            let Value::Bytes(section) = value else { continue };
            let slot = match number {
                field::VERSIONS => &mut settings.versions,
                field::PRIVACY => &mut settings.privacy,
                field::STATUS => &mut settings.status,
                _ => continue,
            };
            *slot = Some(section.to_vec());
        }
        Some(settings)
    }

    /// Takes in a settings update. A partial one replaces only the sections
    /// it holds, as Discord saves them.
    pub fn merge(&mut self, update: UserSettings, partial: bool) {
        if !partial {
            *self = update;
            return;
        }
        let UserSettings { versions, privacy, status } = update;
        self.versions = versions.or(self.versions.take());
        self.privacy = privacy.or(self.privacy.take());
        self.status = status.or(self.status.take());
    }

    /// The version the next edit is based on.
    pub fn data_version(&self) -> u64 {
        self.versions
            .as_deref()
            .and_then(|v| varint_field(v, field::DATA_VERSION))
            .unwrap_or(0)
    }

    /// The status picked on any device; `None` when never picked.
    pub fn status(&self) -> Option<Status> {
        let name = proto::message(self.status.as_deref()?, field::STATUS_NAME).and_then(wrapped_bytes)?;
        Status::ALL.into_iter().find(|s| s.as_str().as_bytes() == name)
    }

    pub fn custom_status(&self) -> Option<CustomStatus> {
        let custom = proto::message(self.status.as_deref()?, field::CUSTOM_STATUS)?;
        let mut status = CustomStatus::default();
        for (number, value) in Reader::new(custom).fields().ok()? {
            match (number, value) {
                (field::TEXT, Value::Bytes(text)) => status.text = String::from_utf8_lossy(text).into_owned(),
                (field::EMOJI_ID, Value::Fixed64(id)) if id != 0 => status.emoji_id = Some(Snowflake(id)),
                (field::EMOJI_NAME, Value::Bytes(name)) if !name.is_empty() => {
                    status.emoji_name = Some(String::from_utf8_lossy(name).into_owned())
                }
                (field::EXPIRES_AT_MS, Value::Fixed64(ms)) => status.expires_at_ms = ms,
                _ => {}
            }
        }
        (!status.is_empty()).then_some(status)
    }

    /// Read as the web client reads it: the newer DM flag wins over the
    /// older one when set.
    pub fn privacy(&self) -> Privacy {
        let section = self.privacy.as_deref().unwrap_or_default();
        let wrapped = |number| proto::message(section, number).and_then(wrapped_varint);
        let dms_restricted = wrapped(field::DEFAULT_GUILDS_RESTRICTED_V2)
            .or_else(|| varint_field(section, field::DEFAULT_GUILDS_RESTRICTED))
            .is_some_and(|v| v != 0);
        Privacy {
            dms_restricted,
            message_requests_restricted: wrapped(field::DEFAULT_MESSAGE_REQUEST_RESTRICTED).is_some_and(|v| v != 0),
            friend_sources: wrapped(field::FRIEND_SOURCE_FLAGS).map_or(friend_source::EVERYONE, |v| v as u32),
        }
    }
}

/// The value of varint field `number` of `bytes`.
fn varint_field(bytes: &[u8], number: u32) -> Option<u64> {
    Reader::new(bytes).fields().ok()?.into_iter().rev().find_map(|(n, value)| match value {
        Value::Varint(v) if n == number => Some(v),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::proto::{Writer, encode_base64};

    fn settings(privacy: Writer, status: Writer) -> UserSettings {
        let mut versions = Writer::default();
        versions.varint(field::DATA_VERSION, 42);
        let mut all = Writer::default();
        all.message(field::VERSIONS, versions)
            .message(field::PRIVACY, privacy)
            .message(field::STATUS, status)
            // A section Emerald does not read.
            .bytes(13, b"\x08\x01");
        UserSettings::decode(&encode_base64(&all.into_bytes())).unwrap()
    }

    #[test]
    fn reads_the_custom_status_and_privacy() {
        let mut custom = Writer::default();
        custom
            .bytes(field::TEXT, "jogando".as_bytes())
            .bytes(field::EMOJI_NAME, "🎮".as_bytes())
            .fixed64(field::EXPIRES_AT_MS, 1000);
        let mut status = Writer::default();
        status.wrapped_string(1, "online").message(field::CUSTOM_STATUS, custom);
        let mut privacy = Writer::default();
        privacy
            .varint(field::DEFAULT_GUILDS_RESTRICTED, 1)
            .wrapped_varint(field::DEFAULT_GUILDS_RESTRICTED_V2, 0)
            .wrapped_varint(field::FRIEND_SOURCE_FLAGS, 6);
        let settings = settings(privacy, status);

        assert_eq!(settings.data_version(), 42);
        assert_eq!(settings.status(), Some(Status::Online));
        let custom = settings.custom_status().unwrap();
        assert_eq!(custom.text, "jogando");
        assert_eq!(custom.emoji_name.as_deref(), Some("🎮"));
        assert!(custom.expired(1000) && !custom.expired(999));
        // The V2 flag, set to false, wins over the old true.
        assert_eq!(
            settings.privacy(),
            Privacy {
                dms_restricted: false,
                message_requests_restricted: false,
                friend_sources: 6
            }
        );
    }

    #[test]
    fn missing_sections_read_as_defaults() {
        let settings = UserSettings::default();
        assert_eq!(settings.custom_status(), None);
        assert_eq!(settings.privacy().friend_sources, friend_source::EVERYONE);
        assert_eq!(settings.data_version(), 0);
        assert_eq!(UserSettings::decode("not base64!"), None);
    }

    #[test]
    fn partial_updates_replace_only_their_sections() {
        let mut privacy = Writer::default();
        privacy.wrapped_varint(field::FRIEND_SOURCE_FLAGS, 2);
        let mut current = settings(privacy, Writer::default());
        let update = UserSettings {
            status: Some(vec![]),
            ..Default::default()
        };
        current.merge(update.clone(), true);
        assert_eq!(current.privacy().friend_sources, 2);
        assert_eq!(current.status.as_deref(), Some(&[][..]));
        current.merge(update, false);
        assert_eq!(current.privacy, None);
    }
}
