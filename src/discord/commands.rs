//! Runs UI commands against the REST API and turns the outcome into the
//! event the UI waits for.

use std::sync::Arc;

use super::protocol::{Action, Command, NetEvent};
use super::rest;

pub(super) async fn handle(rest: Arc<rest::Rest>, cmd: Command, events: async_channel::Sender<NetEvent>) {
    let event = match cmd {
        Command::History { channel, before } => match rest.messages(channel, before).await {
            Ok(messages) => NetEvent::History {
                channel,
                before,
                messages,
            },
            Err(e) if e.is::<rest::Unauthorized>() => NetEvent::AuthExpired,
            Err(e) => NetEvent::HistoryFailed {
                channel,
                forbidden: e.is::<rest::Forbidden>(),
                error: e.to_string(),
            },
        },
        Command::Send {
            channel,
            content,
            nonce,
        } => match with_captcha!(|captcha| rest.send_message(channel, &content, &nonce, captcha).await) {
            // Usually the gateway echo arrives first; the UI drops the duplicate.
            Ok(message) => NetEvent::MessageCreate(message),
            Err(e) if e.is::<rest::Unauthorized>() => NetEvent::AuthExpired,
            Err(e) => NetEvent::SendFailed {
                channel,
                nonce,
                content,
                error: e.to_string(),
            },
        },
        Command::SendFriendRequest { username } => outcome(
            Action::FriendRequest,
            with_captcha!(|captcha| rest.send_friend_request(&username, captcha).await),
        ),
        Command::AcceptFriend { user } => {
            outcome(Action::Relationship, rest.accept_friend(user).await)
        }
        Command::RemoveRelationship { user } => {
            outcome(Action::Relationship, rest.remove_relationship(user).await)
        }
        Command::OpenDm { user, select: true } => {
            match with_captcha!(|captcha| rest.open_dm(user, captcha).await) {
                Ok(channel) => NetEvent::DmOpened {
                    channel,
                    select: true,
                },
                Err(e) => failure(Action::Relationship, e),
            }
        }
        Command::OpenDm { user, select: false } => match rest.open_dm(user, None).await {
            Ok(channel) => NetEvent::DmOpened {
                channel,
                select: false,
            },
            Err(e) => {
                log::warn!("background DM with {user}: {e:#}");
                return;
            }
        },
        Command::JoinGuild { code } => outcome(
            Action::JoinGuild,
            with_captcha!(|captcha| rest.join_guild(&code, captcha).await),
        ),
        Command::CreateGuild { name } => outcome(
            Action::CreateGuild,
            with_captcha!(|captcha| rest.create_guild(&name, captcha).await),
        ),
        Command::SetGlobalName { name } => match rest.set_global_name(name.as_deref()).await {
            Ok(user) => {
                events.try_send(NetEvent::UserUpdate(user)).ok();
                NetEvent::ActionDone(Action::UpdateProfile)
            }
            Err(e) => failure(Action::UpdateProfile, e),
        },
        Command::LogOut => NetEvent::LoggedOut {
            // A token the server already rejects needs no revocation.
            error: match rest.logout().await {
                Err(e) if !e.is::<rest::Unauthorized>() => Some(e.to_string()),
                _ => None,
            },
        },
        Command::Typing { channel } => {
            if let Err(e) = rest.typing(channel).await {
                log::debug!("typing in {channel}: {e:#}");
            }
            return;
        }
        Command::FetchProfile { user, guild } => match rest.profile(user, guild).await {
            Ok(profile) => NetEvent::Profile { guild, profile },
            Err(e) => {
                log::warn!("profile of {user}: {e:#}");
                return;
            }
        },
        Command::FetchAvatar { url } => {
            let bytes = rest
                .cdn_image(&url)
                .await
                .map_err(|e| log::warn!("avatar {url}: {e:#}"))
                .ok();
            NetEvent::Avatar { url, bytes }
        }
        Command::Ack { channel, message } => {
            if let Err(e) = rest.ack(channel, message).await {
                log::debug!("ack {channel}/{message}: {e:#}");
            }
            return;
        }
        // Routed to the gateway before spawning.
        Command::SetStatus(_)
        | Command::RequestMembers { .. }
        | Command::SearchMembers { .. }
        | Command::SubscribeGuild { .. } => {
            return;
        }
    };
    events.try_send(event).ok();
}

/// Evaluates `$call` with `$captcha = None`; if Discord answers with a
/// captcha, shows it to the user and evaluates `$call` once more with their
/// solution. Closing the captcha window keeps the original error. A macro
/// rather than an async closure so the spawned future stays `Send`.
macro_rules! with_captcha {
    (|$captcha:ident| $call:expr) => {{
        let $captcha: Option<&rest::Solution> = None;
        match $call {
            Err(error) => match solve_captcha(&error).await {
                Some(solution) => {
                    let $captcha = Some(&solution);
                    $call
                }
                None => Err(error),
            },
            ok => ok,
        }
    }};
}
use with_captcha;

/// When `error` is a captcha challenge, lets the user solve it in a webview
/// child process and returns the solution.
async fn solve_captcha(error: &anyhow::Error) -> Option<rest::Solution> {
    let rest::CaptchaRequired(challenge) = error.downcast_ref::<rest::CaptchaRequired>()?;
    let challenge = challenge.clone();
    let input = serde_json::to_vec(&challenge).ok()?;
    // The child blocks until its window closes; keep it off the runtime thread.
    let key = tokio::task::spawn_blocking(move || crate::webview::run_child("--captcha", &input))
        .await
        .ok()
        .flatten()?;
    Some(rest::Solution::new(key, &challenge))
}

fn outcome(action: Action, result: anyhow::Result<()>) -> NetEvent {
    match result {
        Ok(()) => NetEvent::ActionDone(action),
        Err(e) => failure(action, e),
    }
}

fn failure(action: Action, error: anyhow::Error) -> NetEvent {
    if error.is::<rest::Unauthorized>() {
        return NetEvent::AuthExpired;
    }
    NetEvent::ActionFailed {
        action,
        error: error.to_string(),
    }
}
