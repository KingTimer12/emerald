//! Notification settings and read states: the bodies of the user guild
//! settings PATCH and of the bulk read receipt.

use serde_json::{Map, Value, json};

use crate::model::Snowflake;
use crate::protocol::SettingsChange;

/// The partial settings `change` stands for, as the web client writes it.
/// `now` (unix seconds) dates the end of a timed mute.
pub fn guild_settings(change: SettingsChange, now: i64) -> Value {
    match change {
        SettingsChange::Mute { channel, seconds } => {
            let mute = mute(seconds, now);
            match channel {
                Some(channel) => overrides(channel, mute),
                None => mute,
            }
        }
        SettingsChange::Notify { channel, level } => {
            let notify = json!({ "message_notifications": level.code() });
            match channel {
                Some(channel) => overrides(channel, notify),
                None => notify,
            }
        }
        SettingsChange::Collapse { category, collapsed } => overrides(category, json!({ "collapsed": collapsed })),
        SettingsChange::HideMuted(hide) => json!({ "hide_muted_channels": hide }),
        SettingsChange::SuppressEveryone(on) => json!({ "suppress_everyone": on }),
        SettingsChange::SuppressRoles(on) => json!({ "suppress_roles": on }),
    }
}

/// Server settings go out through the bulk route, keyed by guild, as the
/// web client saves them.
pub fn bulk(guild: Snowflake, settings: Value) -> Value {
    let mut guilds = Map::new();
    guilds.insert(guild.to_string(), settings);
    json!({ "guilds": guilds })
}

/// `{muted, mute_config}` for a mute of `seconds` (-1: until turned off),
/// or the unmute.
fn mute(seconds: Option<i64>, now: i64) -> Value {
    match seconds {
        None => json!({ "muted": false }),
        Some(seconds) => {
            let end = (seconds > 0).then(|| iso_millis(now + seconds));
            json!({ "muted": true, "mute_config": { "selected_time_window": seconds, "end_time": end } })
        }
    }
}

fn overrides(channel: Snowflake, settings: Value) -> Value {
    let mut channels = Map::new();
    channels.insert(channel.to_string(), settings);
    json!({ "channel_overrides": channels })
}

/// `secs` as JavaScript's `toISOString` writes it: "2025-01-02T14:05:00.000Z".
fn iso_millis(secs: i64) -> String {
    // Days to civil date, after Howard Hinnant's algorithm.
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let (hour, minute, second) = (rest / 3600, rest / 60 % 60, rest % 60);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.000Z")
}

/// `POST /read-states/ack-bulk`: each channel read up to its message.
pub fn ack_bulk(channels: &[(Snowflake, Snowflake)]) -> Value {
    let states: Vec<Value> = channels
        .iter()
        .map(|(channel, message)| {
            json!({ "channel_id": channel.to_string(), "message_id": message.to_string(), "read_state_type": 0 })
        })
        .collect();
    json!({ "read_states": states })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Notify;

    #[test]
    fn mutes_carry_their_window_and_end() {
        let body = guild_settings(
            SettingsChange::Mute {
                channel: None,
                seconds: Some(3600),
            },
            0,
        );
        assert_eq!(
            body,
            json!({ "muted": true, "mute_config": { "selected_time_window": 3600, "end_time": "1970-01-01T01:00:00.000Z" } })
        );
        let forever = guild_settings(
            SettingsChange::Mute {
                channel: Some(Snowflake(5)),
                seconds: Some(-1),
            },
            0,
        );
        assert_eq!(
            forever,
            json!({ "channel_overrides": { "5": { "muted": true, "mute_config": { "selected_time_window": -1, "end_time": null } } } })
        );
        let level = guild_settings(
            SettingsChange::Notify {
                channel: None,
                level: Notify::Nothing,
            },
            0,
        );
        assert_eq!(bulk(Snowflake(1), level), json!({ "guilds": { "1": { "message_notifications": 2 } } }));
    }

    #[test]
    fn bulk_receipts_name_each_channel() {
        assert_eq!(
            ack_bulk(&[(Snowflake(1), Snowflake(2))]),
            json!({ "read_states": [{ "channel_id": "1", "message_id": "2", "read_state_type": 0 }] })
        );
    }
}
