//! Servers: joining through an invite, creating one, inviting people and
//! leaving.

use super::captcha::with_captcha;
use super::outcome::{or_expired, outcome};
use crate::model::Snowflake;
use crate::protocol::{Action, NetEvent};
use crate::rest::{self, Rest};

pub async fn join(rest: &Rest, code: String, message: Option<Snowflake>) -> NetEvent {
    outcome(Action::JoinGuild, with_captcha!(|captcha| rest.join_guild(&code, message, captcha).await))
}

pub async fn create(rest: &Rest, name: String) -> NetEvent {
    outcome(Action::CreateGuild, with_captcha!(|captcha| rest.create_guild(&name, captcha).await))
}

pub async fn create_invite(rest: &Rest, channel: Snowflake, location: &str) -> NetEvent {
    match rest.create_invite(channel, location).await {
        Ok(invite) => NetEvent::InviteCreated { channel, invite: Ok(invite) },
        Err(e) => or_expired(e, |e| NetEvent::InviteCreated {
            channel,
            invite: Err(rest::describe(&e)),
        }),
    }
}

/// A dead invite and a failed lookup both draw as an invalid card.
pub async fn resolve_invite(rest: &Rest, code: String) -> NetEvent {
    let invite = rest
        .invite(&code)
        .await
        .map_err(|e| tracing::info!("invite card: {e:#}"))
        .ok();
    NetEvent::Invite { code, invite }
}

/// The gateway follows with GUILD_DELETE.
pub async fn leave(rest: &Rest, guild: Snowflake) -> NetEvent {
    outcome(Action::LeaveGuild, rest.leave_guild(guild).await)
}
