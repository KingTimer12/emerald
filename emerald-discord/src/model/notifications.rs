//! User guild settings: what the user muted and how much each server,
//! category and channel may notify, as READY's `user_guild_settings` and
//! USER_GUILD_SETTINGS_UPDATE carry them.

use serde::Deserialize;

use super::{Snowflake, iso_secs};

/// `message_notifications`, as the web client numbers it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(from = "u8")]
pub enum Notify {
    All,
    Mentions,
    Nothing,
    /// Follow the level above: the category, then the server.
    #[default]
    Inherit,
}

impl From<u8> for Notify {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::All,
            1 => Self::Mentions,
            2 => Self::Nothing,
            _ => Self::Inherit,
        }
    }
}

impl Notify {
    pub fn code(self) -> u8 {
        match self {
            Self::All => 0,
            Self::Mentions => 1,
            Self::Nothing => 2,
            Self::Inherit => 3,
        }
    }
}

/// How long a mute lasts: its end, if it has one.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct MuteConfig {
    #[serde(default)]
    pub end_time: Option<String>,
    /// Seconds the user picked; -1 for "until I turn it back on".
    #[serde(default)]
    pub selected_time_window: Option<i64>,
}

/// A mute that may run out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mute {
    pub muted: bool,
    /// Unix seconds; `None` lasts until turned off.
    pub until: Option<i64>,
}

impl Mute {
    fn new(muted: bool, config: Option<&MuteConfig>) -> Self {
        let until = config.and_then(|c| iso_secs(c.end_time.as_deref()?));
        Self { muted, until }
    }

    /// Still muted at `now`, as the web client decides it.
    pub fn active(&self, now: i64) -> bool {
        self.muted && self.until.is_none_or(|end| end >= now)
    }
}

/// One server's settings, or the DMs' when `guild_id` is `None`.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(from = "RawGuildSettings")]
pub struct GuildSettings {
    pub guild_id: Option<Snowflake>,
    pub mute: Mute,
    pub notify: Notify,
    pub suppress_everyone: bool,
    pub suppress_roles: bool,
    /// Muted channels stay out of the sidebar.
    pub hide_muted_channels: bool,
    pub channels: Vec<ChannelSettings>,
}

/// A channel's or category's own settings within a server.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChannelSettings {
    pub channel_id: Snowflake,
    pub mute: Mute,
    pub notify: Notify,
    /// Categories: folded in the sidebar.
    pub collapsed: bool,
}

#[derive(Deserialize)]
struct RawGuildSettings {
    #[serde(default)]
    guild_id: Option<Snowflake>,
    #[serde(default)]
    muted: bool,
    #[serde(default)]
    mute_config: Option<MuteConfig>,
    #[serde(default = "inherit")]
    message_notifications: u8,
    #[serde(default)]
    suppress_everyone: bool,
    #[serde(default)]
    suppress_roles: bool,
    #[serde(default)]
    hide_muted_channels: bool,
    /// An array in READY and updates; requests send a map instead.
    #[serde(default)]
    channel_overrides: Vec<RawChannelSettings>,
}

#[derive(Deserialize)]
struct RawChannelSettings {
    channel_id: Snowflake,
    #[serde(default)]
    muted: bool,
    #[serde(default)]
    mute_config: Option<MuteConfig>,
    #[serde(default = "inherit")]
    message_notifications: u8,
    #[serde(default)]
    collapsed: bool,
}

fn inherit() -> u8 {
    Notify::Inherit.code()
}

impl From<RawGuildSettings> for GuildSettings {
    fn from(raw: RawGuildSettings) -> Self {
        Self {
            guild_id: raw.guild_id,
            mute: Mute::new(raw.muted, raw.mute_config.as_ref()),
            notify: raw.message_notifications.into(),
            suppress_everyone: raw.suppress_everyone,
            suppress_roles: raw.suppress_roles,
            hide_muted_channels: raw.hide_muted_channels,
            channels: raw
                .channel_overrides
                .into_iter()
                .map(|c| ChannelSettings {
                    channel_id: c.channel_id,
                    mute: Mute::new(c.muted, c.mute_config.as_ref()),
                    notify: c.message_notifications.into(),
                    collapsed: c.collapsed,
                })
                .collect(),
        }
    }
}

impl GuildSettings {
    pub fn channel(&self, id: Snowflake) -> Option<&ChannelSettings> {
        self.channels.iter().find(|c| c.channel_id == id)
    }

    /// The settings of `id`, added with nothing set when it had none.
    pub fn channel_mut(&mut self, id: Snowflake) -> &mut ChannelSettings {
        match self.channels.iter().position(|c| c.channel_id == id) {
            Some(ix) => &mut self.channels[ix],
            None => {
                self.channels.push(ChannelSettings {
                    channel_id: id,
                    ..ChannelSettings::default()
                });
                self.channels.last_mut().expect("just pushed")
            }
        }
    }
}

/// READY's `user_guild_settings`.
#[derive(Debug, Default, Deserialize)]
pub struct GuildSettingsList {
    #[serde(default)]
    pub entries: Vec<GuildSettings>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_entries_parse_with_their_overrides() {
        let list: GuildSettingsList = serde_json::from_str(
            r#"{"entries":[{"guild_id":"1","muted":true,"mute_config":{"end_time":"2030-01-01T00:00:00.000Z","selected_time_window":3600},
                "message_notifications":1,"suppress_everyone":true,
                "channel_overrides":[{"channel_id":"5","muted":false,"message_notifications":3,"collapsed":true}]},
                {"guild_id":null,"channel_overrides":[{"channel_id":"9","muted":true,"mute_config":null}]}]}"#,
        )
        .unwrap();
        let guild = &list.entries[0];
        assert_eq!(guild.notify, Notify::Mentions);
        assert!(guild.mute.active(0));
        // The hour ran out.
        assert!(!guild.mute.active(iso_secs("2031-01-01T00:00:00").unwrap()));
        assert!(guild.channel(Snowflake(5)).unwrap().collapsed);
        let dms = &list.entries[1];
        assert_eq!(dms.guild_id, None);
        assert!(dms.channel(Snowflake(9)).unwrap().mute.active(i64::MAX));
    }
}
