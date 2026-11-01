//! Bookmarks and reminders: loading them, saving and removing messages,
//! and the clock that notifies when a reminder comes due.

use std::time::Duration;

use super::*;
use emerald_discord::SaveSource;
use emerald_discord::model::{BookmarkPage, SavedList};

/// Longest single wait of the reminder clock; it re-arms after, so a
/// computer that slept does not leave it far behind.
const LONGEST_WAIT: Duration = Duration::from_secs(3600);

impl Emerald {
    pub(super) fn on_saved_messages(&mut self, list: Result<SavedList, String>, cx: &mut Context<Self>) {
        match list {
            Ok(list) => self.saved.load_seen(list, self.settings.reminders_seen_until),
            // As the web client does, a failed load empties the lists.
            Err(error) => {
                tracing::warn!("saved messages: {error}");
                self.saved.load(SavedList::default());
            }
        }
        self.arm_reminders(cx);
    }

    pub(super) fn on_bookmarks(&mut self, before: Option<String>, page: Result<BookmarkPage, String>) {
        match page {
            Ok(page) => self.saved.add_bookmark_page(before, page),
            Err(error) => tracing::warn!("bookmarks: {error}"),
        }
    }

    /// Asks for the next page of bookmarks, or the first one.
    pub(super) fn more_bookmarks(&mut self) {
        if let Some(before) = self.saved.next_bookmark_page() {
            self.net.post(Command::Bookmarks { before });
        }
    }

    /// Bookmarks `message`, or removes its bookmark.
    pub(super) fn toggle_bookmark(&mut self, channel: Snowflake, message: Snowflake, source: SaveSource) {
        let command = if self.saved.is_bookmarked(message) {
            Command::UnsaveMessage {
                channel,
                message,
                reminder: false,
            }
        } else {
            Command::SaveMessage {
                channel,
                message,
                due_secs: None,
                source,
            }
        };
        self.net.post(command);
    }

    /// Sets or moves the reminder of `message` to `due_secs`.
    pub(super) fn set_reminder(&mut self, channel: Snowflake, message: Snowflake, due_secs: i64, source: SaveSource) {
        self.net.post(Command::SaveMessage {
            channel,
            message,
            due_secs: Some(due_secs),
            source,
        });
    }

    /// Removes a bookmark or a reminder; "Mark Complete" too.
    pub(super) fn unsave(&mut self, channel: Snowflake, message: Snowflake) {
        let reminder = self.saved.reminder(message).is_some();
        self.net.post(Command::UnsaveMessage {
            channel,
            message,
            reminder,
        });
    }

    /// Notifies of the reminders already due, then sleeps until the next
    /// one. Each call starts a new clock and retires the previous one, so
    /// nothing runs while no reminder is pending.
    pub(super) fn arm_reminders(&mut self, cx: &mut Context<Self>) {
        let now = time::now_secs();
        let due_now = self.saved.take_due(now);
        if !due_now.is_empty() {
            // Remembered, so the next launch does not notify of them again.
            self.settings.reminders_seen_until = Some(now);
            self.save_settings_soon(cx);
        }
        for due in due_now {
            let body = match (&due.author, &due.summary) {
                (Some(author), Some(summary)) => format!("{author}: {summary}"),
                (_, Some(summary)) => summary.clone(),
                (Some(author), None) => author.clone(),
                (None, None) => String::new(),
            };
            self.alerts.notify(t!("saved.due_notification"), &body, due.channel);
        }
        self.reminder_clock += 1;
        let Some(next) = self.saved.next_due() else { return };
        let clock = self.reminder_clock;
        let wait = Duration::from_secs(next.saturating_sub(now).max(1) as u64).min(LONGEST_WAIT);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            this.update(cx, |this, cx| {
                if this.reminder_clock == clock {
                    this.arm_reminders(cx);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }
}
