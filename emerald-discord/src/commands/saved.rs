//! Bookmarks and reminders. Writes answer with `ActionDone` for the toast;
//! the saved message itself also goes to the store right away, before the
//! gateway's `SAVED_MESSAGE_CREATE` repeats it.

use super::Events;
use super::outcome::{failure, or_expired};
use crate::model::{SavedMessageRemoved, Snowflake};
use crate::packets::saved::SaveSource;
use crate::protocol::{Action, NetEvent};
use crate::rest::{self, Rest};

pub async fn list(rest: &Rest) -> NetEvent {
    match rest.saved_messages().await {
        Ok(list) => NetEvent::SavedMessages(Ok(list)),
        Err(error) => or_expired(error, |error| NetEvent::SavedMessages(Err(rest::describe(&error)))),
    }
}

pub async fn bookmarks(rest: &Rest, before: Option<String>) -> NetEvent {
    match rest.bookmarks(before.as_deref()).await {
        Ok(page) => NetEvent::Bookmarks { before, page: Ok(page) },
        Err(error) => or_expired(error, |error| NetEvent::Bookmarks {
            before,
            page: Err(rest::describe(&error)),
        }),
    }
}

/// Discord words the limit error (code 30074: 50 reminders, 500
/// bookmarks) in the user's language, so its message is shown as is.
pub async fn save(
    rest: &Rest,
    channel: Snowflake,
    message: Snowflake,
    due_secs: Option<i64>,
    source: SaveSource,
    events: &Events,
) -> NetEvent {
    let action = if due_secs.is_some() {
        Action::Reminder { added: true }
    } else {
        Action::Bookmark { added: true }
    };
    match rest.save_message(channel, message, due_secs, source).await {
        Ok(saved) => {
            events.try_send(NetEvent::SavedMessageCreate(Box::new(saved))).ok();
            NetEvent::ActionDone(action)
        }
        Err(error) => failure(action, error),
    }
}

pub async fn unsave(rest: &Rest, channel: Snowflake, message: Snowflake, reminder: bool, events: &Events) -> NetEvent {
    let action = if reminder {
        Action::Reminder { added: false }
    } else {
        Action::Bookmark { added: false }
    };
    match rest.unsave_message(channel, message).await {
        Ok(()) => {
            let removed = SavedMessageRemoved {
                channel_id: channel,
                message_id: message,
            };
            events.try_send(NetEvent::SavedMessageDelete(removed)).ok();
            NetEvent::ActionDone(action)
        }
        Err(error) => failure(action, error),
    }
}
