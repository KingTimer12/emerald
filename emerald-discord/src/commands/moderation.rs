//! Moderation and channel settings: each call reports to the UI as an
//! action, so a failed kick shows why.

use super::captcha::with_captcha;
use super::outcome::outcome;
use crate::model::Snowflake;
use crate::protocol::{Action, ChannelEdit, NetEvent};
use crate::rest::Rest;

pub async fn bulk_delete(rest: &Rest, channel: Snowflake, messages: Vec<Snowflake>) -> NetEvent {
    outcome(
        Action::Moderation,
        with_captcha!(|captcha| rest.bulk_delete(channel, &messages, captcha).await),
    )
}

pub async fn kick(rest: &Rest, guild: Snowflake, user: Snowflake) -> NetEvent {
    outcome(Action::Moderation, with_captcha!(|captcha| rest.kick(guild, user, captcha).await))
}

pub async fn ban(rest: &Rest, guild: Snowflake, user: Snowflake, delete_seconds: u32) -> NetEvent {
    outcome(
        Action::Moderation,
        with_captcha!(|captcha| rest.ban(guild, user, delete_seconds, captcha).await),
    )
}

pub async fn timeout(rest: &Rest, guild: Snowflake, user: Snowflake, until: Option<String>) -> NetEvent {
    outcome(Action::Moderation, rest.timeout(guild, user, until.as_deref()).await)
}

pub async fn edit_channel(rest: &Rest, channel: Snowflake, edit: ChannelEdit) -> NetEvent {
    outcome(Action::ChannelSettings, rest.edit_channel(channel, &edit).await)
}
