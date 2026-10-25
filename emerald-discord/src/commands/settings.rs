//! Notification settings and marking things read.

use super::outcome::outcome;
use crate::model::Snowflake;
use crate::protocol::{Action, NetEvent, SettingsChange};
use crate::rest::Rest;

/// The UI applies the change at once; a failure undoes nothing but is
/// reported, and the gateway's echo settles what Discord kept.
pub async fn guild(rest: &Rest, guild: Option<Snowflake>, change: SettingsChange) -> NetEvent {
    outcome(Action::Settings, rest.guild_settings(guild, change).await)
}

/// Read receipts fail quietly, like single ones.
pub async fn ack_bulk(rest: &Rest, channels: Vec<(Snowflake, Snowflake)>) -> Option<NetEvent> {
    if let Err(e) = rest.ack_bulk(&channels).await {
        tracing::debug!("bulk ack of {} channels: {e:#}", channels.len());
    }
    None
}
