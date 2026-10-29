//! What the user muted and what each place may notify of, from their user
//! guild settings, and the web client's rules for whether a message makes
//! a desktop notification.

use rustc_hash::FxHashMap;

use emerald_discord::SettingsChange;
use emerald_discord::model::{GuildSettings, Mute, Notify, Snowflake};

/// Message flag Discord sets on `@silent` messages.
const SUPPRESS_NOTIFICATIONS: u64 = 1 << 12;

/// Where a message landed: the server (none for DMs) and the channel with
/// the levels above it, nearest first (a thread's channel, then its
/// category).
#[derive(Clone, Debug, Default)]
pub struct Place {
    pub guild: Option<Snowflake>,
    /// The channel itself, then its parents.
    pub chain: Vec<Snowflake>,
}

/// What a message says that decides whether it notifies.
#[derive(Clone, Copy, Debug, Default)]
pub struct Ping {
    /// Names the user directly.
    pub user: bool,
    pub everyone: bool,
    /// Names one of the user's roles.
    pub role: bool,
    pub flags: u64,
}

#[derive(Default)]
pub struct NotifySettings {
    /// Per guild, and the DMs' under `None`.
    guilds: FxHashMap<Option<Snowflake>, GuildSettings>,
    /// The guild's own default when the user set no level.
    defaults: FxHashMap<Snowflake, Notify>,
}

impl NotifySettings {
    /// A new session's settings and each guild's default level.
    pub fn load(&mut self, settings: Vec<GuildSettings>, defaults: impl Iterator<Item = (Snowflake, Notify)>) {
        self.guilds = settings.into_iter().map(|s| (s.guild_id, s)).collect();
        self.defaults = defaults.collect();
    }

    pub fn set_default(&mut self, guild: Snowflake, level: Notify) {
        self.defaults.insert(guild, level);
    }

    /// USER_GUILD_SETTINGS_UPDATE: the whole of one guild's settings.
    pub fn update(&mut self, settings: GuildSettings) {
        self.guilds.insert(settings.guild_id, settings);
    }

    /// Applies `change` at once, before Discord confirms it; `now` dates
    /// the end of a timed mute.
    pub fn apply(&mut self, guild: Option<Snowflake>, change: SettingsChange, now: i64) {
        let settings = self.guilds.entry(guild).or_insert_with(|| GuildSettings {
            guild_id: guild,
            ..GuildSettings::default()
        });
        match change {
            SettingsChange::Mute { channel, seconds } => {
                let mute = Mute {
                    muted: seconds.is_some(),
                    until: seconds.filter(|s| *s > 0).map(|s| now + s),
                };
                match channel {
                    Some(channel) => settings.channel_mut(channel).mute = mute,
                    None => settings.mute = mute,
                }
            }
            SettingsChange::Notify { channel, level } => match channel {
                Some(channel) => settings.channel_mut(channel).notify = level,
                None => settings.notify = level,
            },
            SettingsChange::Collapse { category, collapsed } => settings.channel_mut(category).collapsed = collapsed,
            SettingsChange::HideMuted(hide) => settings.hide_muted_channels = hide,
            SettingsChange::SuppressEveryone(on) => settings.suppress_everyone = on,
            SettingsChange::SuppressRoles(on) => settings.suppress_roles = on,
        }
    }

    pub fn guild(&self, guild: Option<Snowflake>) -> Option<&GuildSettings> {
        self.guilds.get(&guild)
    }

    /// The server itself is muted.
    pub fn guild_muted(&self, guild: Snowflake, now: i64) -> bool {
        self.guilds.get(&Some(guild)).is_some_and(|g| g.mute.active(now))
    }

    /// `channel` alone is muted, not counting its server or parents.
    pub fn channel_muted(&self, guild: Option<Snowflake>, channel: Snowflake, now: i64) -> bool {
        self.guild(guild).and_then(|g| g.channel(channel)).is_some_and(|c| c.mute.active(now))
    }

    /// Muted by the server, the channel or any level between.
    pub fn muted(&self, place: &Place, now: i64) -> bool {
        place.guild.is_some_and(|guild| self.guild_muted(guild, now))
            || place.chain.iter().any(|channel| self.channel_muted(place.guild, *channel, now))
    }

    /// The mute set on the server, or on `channel` when given.
    pub fn mute(&self, guild: Option<Snowflake>, channel: Option<Snowflake>) -> Mute {
        let Some(settings) = self.guild(guild) else { return Mute::default() };
        match channel {
            Some(channel) => settings.channel(channel).map(|c| c.mute).unwrap_or_default(),
            None => settings.mute,
        }
    }

    /// The level set on the server, or on `channel` (maybe `Inherit`).
    pub fn level(&self, guild: Option<Snowflake>, channel: Option<Snowflake>) -> Notify {
        let Some(settings) = self.guild(guild) else { return Notify::Inherit };
        match channel {
            Some(channel) => settings.channel(channel).map(|c| c.notify).unwrap_or_default(),
            None => settings.notify,
        }
    }

    /// The level that applies at `place`: the nearest one set, else the
    /// server's, else the server's default. DMs notify of everything.
    pub fn resolved(&self, place: &Place) -> Notify {
        let Some(guild) = place.guild else { return Notify::All };
        place
            .chain
            .iter()
            .map(|channel| self.level(place.guild, Some(*channel)))
            .chain([self.level(place.guild, None)])
            .find(|level| *level != Notify::Inherit)
            .unwrap_or_else(|| self.defaults.get(&guild).copied().unwrap_or(Notify::Mentions))
    }

    pub fn is_collapsed(&self, guild: Snowflake, category: Snowflake) -> bool {
        self.guild(Some(guild)).and_then(|g| g.channel(category)).is_some_and(|c| c.collapsed)
    }

    pub fn hides_muted(&self, guild: Snowflake) -> bool {
        self.guild(Some(guild)).is_some_and(|g| g.hide_muted_channels)
    }

    /// Whether a message at `place` makes a desktop notification, as the
    /// web client decides: never when muted, set to nothing or sent
    /// silently; always at "all messages"; otherwise only when it pings
    /// the user, @everyone and roles unless the server suppresses them.
    pub fn should_notify(&self, place: &Place, ping: Ping, now: i64) -> bool {
        if ping.flags & SUPPRESS_NOTIFICATIONS != 0 || self.muted(place, now) {
            return false;
        }
        match self.resolved(place) {
            Notify::Nothing => false,
            Notify::All | Notify::Inherit => true,
            Notify::Mentions => {
                let settings = self.guild(place.guild);
                let everyone = ping.everyone && !settings.is_some_and(|s| s.suppress_everyone);
                let role = ping.role && !settings.is_some_and(|s| s.suppress_roles);
                ping.user || everyone || role
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GUILD: Snowflake = Snowflake(1);
    const CATEGORY: Snowflake = Snowflake(2);
    const CHANNEL: Snowflake = Snowflake(3);

    fn place() -> Place {
        Place {
            guild: Some(GUILD),
            chain: vec![CHANNEL, CATEGORY],
        }
    }

    fn settings(defaults: Notify) -> NotifySettings {
        let mut settings = NotifySettings::default();
        settings.load(Vec::new(), [(GUILD, defaults)].into_iter());
        settings
    }

    const USER: Ping = Ping {
        user: true,
        everyone: false,
        role: false,
        flags: 0,
    };
    const EVERYONE: Ping = Ping {
        user: false,
        everyone: true,
        role: false,
        flags: 0,
    };
    const PLAIN: Ping = Ping {
        user: false,
        everyone: false,
        role: false,
        flags: 0,
    };

    #[test]
    fn levels_inherit_from_the_nearest_set() {
        let mut settings = settings(Notify::Mentions);
        assert!(!settings.should_notify(&place(), PLAIN, 0));
        assert!(settings.should_notify(&place(), USER, 0));
        settings.apply(Some(GUILD), SettingsChange::Notify { channel: Some(CATEGORY), level: Notify::All }, 0);
        assert!(settings.should_notify(&place(), PLAIN, 0));
        settings.apply(Some(GUILD), SettingsChange::Notify { channel: Some(CHANNEL), level: Notify::Nothing }, 0);
        assert!(!settings.should_notify(&place(), USER, 0));
        // DMs notify of everything.
        assert!(settings.should_notify(&Place { guild: None, chain: vec![Snowflake(9)] }, PLAIN, 0));
    }

    #[test]
    fn mutes_silence_until_they_run_out() {
        let mut settings = settings(Notify::All);
        settings.apply(Some(GUILD), SettingsChange::Mute { channel: Some(CATEGORY), seconds: Some(900) }, 100);
        assert!(settings.muted(&place(), 500));
        assert!(!settings.should_notify(&place(), USER, 500));
        // Fifteen minutes later it is over.
        assert!(settings.should_notify(&place(), PLAIN, 1001));
        settings.apply(Some(GUILD), SettingsChange::Mute { channel: None, seconds: Some(-1) }, 0);
        assert!(settings.guild_muted(GUILD, i64::MAX));
        settings.apply(Some(GUILD), SettingsChange::Mute { channel: None, seconds: None }, 0);
        assert!(!settings.guild_muted(GUILD, 0));
    }

    #[test]
    fn suppressed_pings_and_silent_messages_stay_quiet() {
        let mut settings = settings(Notify::Mentions);
        assert!(settings.should_notify(&place(), EVERYONE, 0));
        settings.apply(Some(GUILD), SettingsChange::SuppressEveryone(true), 0);
        assert!(!settings.should_notify(&place(), EVERYONE, 0));
        let silent = Ping { flags: SUPPRESS_NOTIFICATIONS, ..USER };
        assert!(!settings.should_notify(&place(), silent, 0));
    }

    #[test]
    fn collapsed_categories_come_from_overrides() {
        let mut settings = settings(Notify::All);
        settings.apply(Some(GUILD), SettingsChange::Collapse { category: CATEGORY, collapsed: true }, 0);
        assert!(settings.is_collapsed(GUILD, CATEGORY));
        assert!(!settings.is_collapsed(GUILD, CHANNEL));
    }
}
