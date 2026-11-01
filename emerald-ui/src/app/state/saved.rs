//! Bookmarks and reminders as Discord keeps them: which messages are
//! saved, the reminders by due time, the pages of bookmarks loaded for the
//! Inbox, and which reminders have already notified.

use emerald_discord::model::{BookmarkPage, SaveData, SavedList, SavedMessage, Snowflake};
use rustc_hash::FxHashSet;

use super::Msg;

/// A saved message as the Inbox lists it: what Discord keeps about the
/// save, and the message when it can still be read.
pub struct Entry {
    pub data: SaveData,
    pub msg: Option<Msg>,
}

impl Entry {
    /// When the reminder is due, in Unix seconds; `None` for a bookmark.
    pub fn due_secs(&self) -> Option<i64> {
        emerald_discord::model::iso_secs(self.data.due_at.as_deref()?)
    }
}

impl From<SavedMessage> for Entry {
    fn from(saved: SavedMessage) -> Self {
        Self {
            data: saved.save_data,
            msg: saved.message.map(Msg::from),
        }
    }
}

/// A reminder that came due, as the notification needs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Due {
    pub channel: Snowflake,
    pub guild: Option<Snowflake>,
    pub message: Snowflake,
    pub author: Option<String>,
    pub summary: Option<String>,
}

#[derive(Default)]
pub struct Saved {
    bookmark_ids: FxHashSet<Snowflake>,
    /// Sorted by due time, soonest (and so overdue) first.
    reminders: Vec<Entry>,
    /// Bookmarks loaded so far, newest first.
    bookmarks: Vec<Entry>,
    /// `saved_at` of the last loaded bookmark: the next page's cursor.
    next_before: Option<String>,
    has_more: bool,
    bookmarks_asked: bool,
    /// Reminders that already notified, by message and due time, so a
    /// reminder moved to a new time notifies again.
    fired: FxHashSet<(Snowflake, i64)>,
}

impl Saved {
    /// [`Saved::load`], with the reminders due by `seen_until` (Unix
    /// seconds, when the last session ended) counted as notified already.
    pub fn load_seen(&mut self, list: SavedList, seen_until: Option<i64>) {
        self.load(list);
        let Some(seen) = seen_until else { return };
        for entry in &self.reminders {
            if let Some(due) = entry.due_secs().filter(|due| *due <= seen) {
                self.fired.insert((entry.data.message_id, due));
            }
        }
    }

    pub fn load(&mut self, list: SavedList) {
        self.bookmark_ids = list.bookmark_ids.into_iter().collect();
        self.reminders = list.reminders.into_iter().map(Entry::from).collect();
        self.sort();
        // Pages loaded before a refetch may be out of date.
        self.bookmarks.clear();
        self.next_before = None;
        self.has_more = false;
        self.bookmarks_asked = false;
    }

    /// A message saved here or elsewhere: a new bookmark, or a reminder set
    /// or moved. Saving a message replaces whatever it was before.
    pub fn upsert(&mut self, saved: SavedMessage) {
        let (channel, message) = (saved.save_data.channel_id, saved.save_data.message_id);
        self.remove(channel, message);
        if saved.is_reminder() {
            self.reminders.push(saved.into());
            self.sort();
        } else {
            self.bookmark_ids.insert(message);
            // Newest first; anything older is still on the server's pages.
            if self.bookmarks_asked {
                self.bookmarks.insert(0, saved.into());
            }
        }
    }

    pub fn remove(&mut self, channel: Snowflake, message: Snowflake) {
        let other = |s: &Entry| (s.data.channel_id, s.data.message_id) != (channel, message);
        self.bookmark_ids.remove(&message);
        self.reminders.retain(other);
        self.bookmarks.retain(other);
    }

    pub fn is_bookmarked(&self, message: Snowflake) -> bool {
        self.bookmark_ids.contains(&message)
    }

    pub fn reminder(&self, message: Snowflake) -> Option<&Entry> {
        self.reminders.iter().find(|s| s.data.message_id == message)
    }

    pub fn reminders(&self) -> &[Entry] {
        &self.reminders
    }

    pub fn bookmarks(&self) -> &[Entry] {
        &self.bookmarks
    }

    pub fn has_more_bookmarks(&self) -> bool {
        self.has_more
    }

    /// The cursor of the bookmark page to ask for next: `Some(None)` for the
    /// first page, `None` once nothing more is left.
    pub fn next_bookmark_page(&mut self) -> Option<Option<String>> {
        if !self.bookmarks_asked {
            self.bookmarks_asked = true;
            return Some(None);
        }
        self.has_more.then(|| self.next_before.clone())
    }

    pub fn add_bookmark_page(&mut self, before: Option<String>, page: BookmarkPage) {
        if before.is_none() {
            self.bookmarks.clear();
        } else if before != self.next_before {
            // A stale answer, for a cursor that no longer matches.
            return;
        }
        self.has_more = page.has_more;
        if let Some(last) = page.results.last() {
            self.next_before = Some(last.save_data.saved_at.clone());
        }
        for saved in page.results {
            self.bookmark_ids.insert(saved.save_data.message_id);
            self.bookmarks.push(saved.into());
        }
    }

    /// Reminders whose time has passed.
    pub fn overdue(&self, now_secs: i64) -> usize {
        self.reminders.iter().filter(|s| s.due_secs().is_some_and(|due| due <= now_secs)).count()
    }

    /// When the next reminder that has not notified yet comes due.
    pub fn next_due(&self) -> Option<i64> {
        self.reminders
            .iter()
            .filter_map(|s| Some((s.data.message_id, s.due_secs()?)))
            .find(|key| !self.fired.contains(key))
            .map(|(_, due)| due)
    }

    /// The reminders due by `now_secs` that have not notified yet; each
    /// comes back once.
    pub fn take_due(&mut self, now_secs: i64) -> Vec<Due> {
        let mut due = Vec::new();
        for saved in &self.reminders {
            let Some(at) = saved.due_secs() else { continue };
            if at > now_secs {
                break;
            }
            let data = &saved.data;
            if self.fired.insert((data.message_id, at)) {
                due.push(Due {
                    channel: data.channel_id,
                    guild: data.guild_id,
                    message: data.message_id,
                    author: data.author_summary.clone(),
                    summary: data.message_summary.clone(),
                });
            }
        }
        due
    }

    fn sort(&mut self) {
        self.reminders.sort_by_key(|s| s.due_secs().unwrap_or(i64::MAX));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saved(message: u64, due: Option<&str>) -> SavedMessage {
        let due = due.map_or("null".to_owned(), |d| format!("\"{d}\""));
        serde_json::from_str(&format!(
            r#"{{"message":null,"save_data":{{"channel_id":"1","message_id":"{message}",
            "saved_at":"2026-10-30T10:00:00+00:00","guild_id":0,"author_id":0,"due_at":{due}}}}}"#
        ))
        .unwrap()
    }

    /// 2026-10-30T12:00:00Z.
    const NOON: i64 = 1_793_361_600;

    #[test]
    fn bookmark_then_remove() {
        let mut store = Saved::default();
        store.upsert(saved(5, None));
        assert!(store.is_bookmarked(Snowflake(5)));
        store.remove(Snowflake(1), Snowflake(5));
        assert!(!store.is_bookmarked(Snowflake(5)));
    }

    #[test]
    fn reminders_sorted_by_due() {
        let mut store = Saved::default();
        store.upsert(saved(1, Some("2026-10-30T13:00:00+00:00")));
        store.upsert(saved(2, Some("2026-10-30T12:00:00+00:00")));
        let order: Vec<u64> = store.reminders().iter().map(|s| s.data.message_id.0).collect();
        assert_eq!(order, [2, 1]);
        assert_eq!(store.next_due(), Some(NOON));
        // Saving it as a bookmark replaces the reminder.
        store.upsert(saved(2, None));
        assert_eq!(store.reminders().len(), 1);
        assert!(store.is_bookmarked(Snowflake(2)));
    }

    #[test]
    fn due_fires_once() {
        let mut store = Saved::default();
        store.load(SavedList {
            reminders: vec![saved(1, Some("2026-10-30T12:00:00+00:00")), saved(2, Some("2026-10-30T14:00:00+00:00"))],
            bookmark_ids: vec![],
        });
        assert!(store.take_due(NOON - 1).is_empty());
        // Overdue on start: fires once, then never again.
        let due = store.take_due(NOON + 60);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].message, Snowflake(1));
        assert!(store.take_due(NOON + 120).is_empty());
        assert_eq!(store.next_due(), Some(NOON + 7200));
        assert_eq!(store.overdue(NOON + 60), 1);
        // Moved to a later time, it notifies again then.
        store.upsert(saved(1, Some("2026-10-30T15:00:00+00:00")));
        assert_eq!(store.take_due(NOON + 3 * 3600).len(), 2);
    }

    #[test]
    fn reminders_seen_last_session_do_not_fire_again() {
        let mut store = Saved::default();
        let list = || SavedList {
            reminders: vec![saved(1, Some("2026-10-30T12:00:00+00:00")), saved(2, Some("2026-10-30T14:00:00+00:00"))],
            bookmark_ids: vec![],
        };
        // The last session ran until 13:00: the noon one already notified.
        store.load_seen(list(), Some(NOON + 3600));
        let due = store.take_due(NOON + 3 * 3600);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].message, Snowflake(2));
    }

    #[test]
    fn deleted_before_due_never_fires() {
        let mut store = Saved::default();
        store.upsert(saved(1, Some("2026-10-30T12:00:00+00:00")));
        store.remove(Snowflake(1), Snowflake(1));
        assert!(store.take_due(NOON + 60).is_empty());
        assert_eq!(store.next_due(), None);
    }

    #[test]
    fn bookmark_pages_follow_the_cursor() {
        let mut store = Saved::default();
        assert_eq!(store.next_bookmark_page(), Some(None));
        store.add_bookmark_page(None, BookmarkPage { results: vec![saved(3, None)], has_more: true });
        let cursor = store.next_bookmark_page().unwrap();
        assert_eq!(cursor.as_deref(), Some("2026-10-30T10:00:00+00:00"));
        store.add_bookmark_page(cursor, BookmarkPage { results: vec![saved(4, None)], has_more: false });
        assert_eq!(store.bookmarks().len(), 2);
        assert_eq!(store.next_bookmark_page(), None);
    }
}
