//! Servers: joining through an invite and creating one.

use super::captcha::with_captcha;
use super::outcome::outcome;
use crate::protocol::{Action, NetEvent};
use crate::rest::Rest;

pub async fn join(rest: &Rest, code: String) -> NetEvent {
    outcome(Action::JoinGuild, with_captcha!(|captcha| rest.join_guild(&code, captcha).await))
}

pub async fn create(rest: &Rest, name: String) -> NetEvent {
    outcome(Action::CreateGuild, with_captcha!(|captcha| rest.create_guild(&name, captcha).await))
}
