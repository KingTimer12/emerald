//! The account's own settings, profile and signed-in devices.

use super::Events;
use super::outcome::{failure, or_expired};
use crate::model::{Snowflake, UserSettings};
use crate::protocol::{Action, NetEvent, ProfileEdit, SettingsEdit};
use crate::rest::Rest;

/// The saved settings reach the UI before the action is done; an edit made
/// against an older version is dropped by Discord and reported.
pub async fn edit_settings(rest: &Rest, current: UserSettings, edit: SettingsEdit, events: &Events) -> NetEvent {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    match rest.edit_settings(&current, &edit, now_ms).await {
        Ok(saved) => {
            if let Some(settings) = saved.settings {
                events.try_send(NetEvent::UserSettings { settings, partial: false }).ok();
            }
            if saved.out_of_date {
                NetEvent::ActionFailed {
                    action: Action::AccountSettings,
                    error: t!("errors.settings_out_of_date").to_owned(),
                }
            } else {
                NetEvent::ActionDone(Action::AccountSettings)
            }
        }
        Err(e) => failure(Action::AccountSettings, e),
    }
}

/// The name and avatar go to the user (or the membership in `guild`), the
/// rest to the profile; each part reaches the UI as it is saved.
pub async fn edit_profile(rest: &Rest, guild: Option<Snowflake>, edit: ProfileEdit, events: &Events) -> NetEvent {
    if edit.touches_user() {
        let saved = match guild {
            Some(guild) => rest.edit_member(guild, &edit).await.map(drop),
            // The gateway echoes a member; the user only comes back here.
            None => rest.edit_user(&edit).await.map(|user| {
                events.try_send(NetEvent::UserUpdate(user)).ok();
            }),
        };
        if let Err(e) = saved {
            return failure(Action::UpdateProfile, e);
        }
    }
    if edit.touches_profile() {
        match rest.edit_profile(guild, &edit).await {
            Ok(details) => {
                events.try_send(NetEvent::ProfileEdited { guild, details }).ok();
            }
            Err(e) => return failure(Action::UpdateProfile, e),
        }
    }
    NetEvent::ActionDone(Action::UpdateProfile)
}

pub async fn sessions(rest: &Rest) -> NetEvent {
    match rest.sessions().await {
        Ok(sessions) => NetEvent::Sessions(Ok(sessions)),
        Err(e) => or_expired(e, |e| NetEvent::Sessions(Err(crate::rest::describe(&e)))),
    }
}

/// Discord may first ask the user to confirm it is them; that comes back
/// as a failure, never as a sign-out.
pub async fn log_out_sessions(rest: &Rest, hashes: Vec<String>) -> NetEvent {
    match rest.log_out_sessions(&hashes).await {
        Ok(()) => NetEvent::SessionsLoggedOut(hashes),
        Err(e) => failure(Action::Sessions, e),
    }
}
