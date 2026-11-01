//! Bodies for saving messages as bookmarks and reminders.

use serde_json::{Map, Value, json};

/// Where in the interface a message was saved, as the web client names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveSource {
    ContextMenu,
    MessageToolbar,
    ForLaterList,
    NotificationsInbox,
}

impl SaveSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ContextMenu => "context_menu",
            Self::MessageToolbar => "message_toolbar",
            Self::ForLaterList => "for_later_list",
            Self::NotificationsInbox => "notifications_inbox",
        }
    }
}

/// `PUT /users/@me/saved-messages/{c}/{m}`: `due_at` is left out for a
/// bookmark, and written as JavaScript's `toISOString` for a reminder.
pub fn save(due_secs: Option<i64>, source: SaveSource) -> Value {
    let mut body = Map::new();
    if let Some(secs) = due_secs {
        body.insert("due_at".into(), json!(super::settings::iso_millis(secs)));
    }
    body.insert("source".into(), json!(source.as_str()));
    Value::Object(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bookmarks_leave_due_at_out() {
        assert_eq!(save(None, SaveSource::ContextMenu).to_string(), r#"{"source":"context_menu"}"#);
        assert_eq!(
            save(Some(1_793_361_600), SaveSource::MessageToolbar).to_string(),
            r#"{"due_at":"2026-10-30T12:00:00.000Z","source":"message_toolbar"}"#
        );
    }
}
