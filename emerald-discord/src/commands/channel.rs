//! Private channels: group DMs and their members, closing a DM and
//! answering message requests. The gateway echoes most outcomes as channel
//! and recipient events.

use super::captcha::with_captcha;
use super::outcome::{failure, outcome};
use crate::model::Snowflake;
use crate::protocol::{Action, NetEvent};
use crate::rest::Rest;

/// The new group opens once made.
pub async fn create_group(rest: &Rest, users: Vec<Snowflake>) -> NetEvent {
    match with_captcha!(|captcha| rest.create_group(&users, captcha).await) {
        Ok(channel) => NetEvent::DmOpened { channel, select: true },
        Err(e) => failure(Action::Channel, e),
    }
}

/// The first addition to a 1:1 DM makes a new group, which the others then
/// join and which opens, as the web client does.
pub async fn add_recipients(rest: &Rest, mut channel: Snowflake, users: Vec<Snowflake>) -> NetEvent {
    let mut created = None;
    for user in users {
        match rest.add_recipient(channel, user).await {
            Ok(Some(group)) => {
                channel = group.id;
                created = Some(group);
            }
            Ok(None) => {}
            Err(e) => return failure(Action::Channel, e),
        }
    }
    match created {
        Some(channel) => NetEvent::DmOpened { channel, select: true },
        None => NetEvent::ActionDone(Action::Channel),
    }
}

pub async fn remove_recipient(rest: &Rest, channel: Snowflake, user: Snowflake) -> NetEvent {
    outcome(Action::Channel, rest.remove_recipient(channel, user).await)
}

pub async fn edit_group(rest: &Rest, channel: Snowflake, body: serde_json::Value) -> NetEvent {
    outcome(Action::Channel, rest.edit_group(channel, body).await)
}

pub async fn close(rest: &Rest, channel: Snowflake) -> NetEvent {
    outcome(Action::Channel, rest.close_channel(channel).await)
}

pub async fn answer_request(rest: &Rest, channel: Snowflake, accept: bool) -> NetEvent {
    let result = if accept { rest.accept_request(channel).await } else { rest.reject_request(channel).await };
    outcome(Action::Channel, result)
}
