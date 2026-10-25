//! Turning REST results into events. Every command treats a rejected token
//! the same way (sign in again), so [`or_expired`] does that once.

use crate::protocol::{Action, NetEvent};
use crate::rest::{self, Unauthorized};

/// `AuthExpired` when Discord rejected the token, else what `other` makes
/// of the error.
pub fn or_expired(error: anyhow::Error, other: impl FnOnce(anyhow::Error) -> NetEvent) -> NetEvent {
    if error.is::<Unauthorized>() {
        return NetEvent::AuthExpired;
    }
    other(error)
}

/// `ActionDone`, or the failure of `action`.
pub fn outcome<T>(action: Action, result: anyhow::Result<T>) -> NetEvent {
    match result {
        Ok(_) => NetEvent::ActionDone(action),
        Err(error) => failure(action, error),
    }
}

/// `ActionFailed` with the error worded for the user.
pub fn failure(action: Action, error: anyhow::Error) -> NetEvent {
    or_expired(error, |error| NetEvent::ActionFailed {
        action,
        error: rest::describe(&error),
    })
}

/// The host of `url`, for logs: paths can name channels and files.
pub fn host(url: &str) -> &str {
    url.split('/').nth(2).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejected_tokens_expire_the_session() {
        let event = failure(Action::Message, Unauthorized.into());
        assert!(matches!(event, NetEvent::AuthExpired));
        let event = failure(Action::Message, anyhow::anyhow!("nope"));
        assert!(matches!(event, NetEvent::ActionFailed { action: Action::Message, .. }));
    }

    #[test]
    fn logs_keep_only_the_host() {
        assert_eq!(host("https://cdn.discordapp.com/attachments/1/2/a.gif"), "cdn.discordapp.com");
        assert_eq!(host("nonsense"), "");
    }
}
