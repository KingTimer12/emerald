//! The account and the people around it: friends, blocks, nicknames,
//! notes, DMs, profiles, the display name and logging out.

use super::Events;
use super::captcha::with_captcha;
use super::outcome::{failure, outcome};
use crate::model::Snowflake;
use crate::protocol::{Action, NetEvent};
use crate::rest::{Rest, Unauthorized};

pub async fn friend_request(rest: &Rest, username: String) -> NetEvent {
    outcome(Action::FriendRequest, with_captcha!(|captcha| rest.send_friend_request(&username, captcha).await))
}

pub async fn accept_friend(rest: &Rest, user: Snowflake) -> NetEvent {
    outcome(Action::Relationship, rest.accept_friend(user).await)
}

/// Removes a friend, declines an incoming request or cancels an outgoing one.
pub async fn remove_relationship(rest: &Rest, user: Snowflake) -> NetEvent {
    outcome(Action::Relationship, rest.remove_relationship(user, "Friends").await)
}

/// The gateway echoes the new relationship, or its removal.
pub async fn block(rest: &Rest, user: Snowflake, block: bool) -> NetEvent {
    let result = if block { rest.block(user).await } else { rest.remove_relationship(user, "ContextMenu").await };
    outcome(Action::Relationship, result)
}

pub async fn ignore(rest: &Rest, user: Snowflake, ignore: bool) -> NetEvent {
    outcome(Action::Relationship, rest.ignore(user, ignore).await)
}

pub async fn set_nickname(rest: &Rest, user: Snowflake, nickname: Option<String>) -> NetEvent {
    outcome(Action::Relationship, rest.set_nickname(user, nickname.as_deref()).await)
}

/// A note that cannot be read is left out, not shown as empty.
pub async fn note(rest: &Rest, user: Snowflake) -> Option<NetEvent> {
    match rest.note(user).await {
        Ok(note) => Some(NetEvent::Note { user, note }),
        Err(e) => {
            tracing::warn!("note on {user}: {e:#}");
            None
        }
    }
}

/// The gateway echoes the note as USER_NOTE_UPDATE.
pub async fn set_note(rest: &Rest, user: Snowflake, note: String) -> NetEvent {
    outcome(Action::Note, rest.set_note(user, &note).await)
}

pub async fn mutual_friends(rest: &Rest, user: Snowflake) -> NetEvent {
    let friends = rest
        .mutual_friends(user)
        .await
        .map_err(|e| tracing::warn!("mutual friends with {user}: {e:#}"))
        .ok();
    NetEvent::MutualFriends { user, friends }
}

/// A DM to select runs as a user action, captcha included; one opened in
/// the background never shows a captcha and fails silently.
pub async fn open_dm(rest: &Rest, user: Snowflake, select: bool) -> Option<NetEvent> {
    if select {
        return Some(match with_captcha!(|captcha| rest.open_dm(user, captcha).await) {
            Ok(channel) => NetEvent::DmOpened { channel, select },
            Err(e) => failure(Action::Relationship, e),
        });
    }
    match rest.open_dm(user, None).await {
        Ok(channel) => Some(NetEvent::DmOpened { channel, select }),
        Err(e) => {
            tracing::warn!("background DM with {user}: {e:#}");
            None
        }
    }
}

/// The new name reaches the UI as a `UserUpdate` before the action is done.
pub async fn set_global_name(rest: &Rest, name: Option<String>, events: &Events) -> NetEvent {
    match rest.set_global_name(name.as_deref()).await {
        Ok(user) => {
            events.try_send(NetEvent::UserUpdate(user)).ok();
            NetEvent::ActionDone(Action::UpdateProfile)
        }
        Err(e) => failure(Action::UpdateProfile, e),
    }
}

pub async fn profile(rest: &Rest, user: Snowflake, guild: Option<Snowflake>, full: bool) -> Option<NetEvent> {
    match rest.profile(user, guild, full).await {
        Ok(profile) => Some(NetEvent::Profile { guild, profile }),
        Err(e) => {
            tracing::warn!("profile of {user}: {e:#}");
            None
        }
    }
}

pub async fn log_out(rest: &Rest) -> NetEvent {
    NetEvent::LoggedOut {
        // A token the server already rejects needs no revocation.
        error: match rest.logout().await {
            Err(e) if !e.is::<Unauthorized>() => Some(format!("{e:#}")),
            _ => None,
        },
    }
}
