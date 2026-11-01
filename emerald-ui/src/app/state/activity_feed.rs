//! The content inventory feed as the member list's Activity section uses
//! it: when to ask for it again, the backoff after failures, and which
//! entries a server's list shows.

use emerald_discord::model::{Content, Feed, Snowflake, content_type};

/// Waits after the first failures, in milliseconds: 1, 2, 4 and 8 minutes.
const BACKOFF_MS: [u64; 4] = [60_000, 120_000, 240_000, 480_000];
/// When Discord names no wait, the feed is asked for again after this.
const DEFAULT_WAIT_MS: u64 = 10 * 60_000;

/// Entry kinds the member list shows.
const SHOWN: [u32; 6] = [
    content_type::PLAYED_GAME,
    content_type::WATCHED_MEDIA,
    content_type::TOP_GAME,
    content_type::TOP_ARTIST,
    content_type::LISTENED_SESSION,
    content_type::LAUNCHED_ACTIVITY,
];

#[derive(Default)]
pub struct ActivityFeed {
    entries: Vec<Content>,
    refresh_token: Option<String>,
    /// When the feed may be asked for again, in Unix ms; `None` before the
    /// first answer, when it is due at once.
    next_fetch_ms: Option<u64>,
    failures: usize,
    /// A request is out.
    asking: bool,
}

impl ActivityFeed {
    /// Whether to ask now: not while a request is out, nor before its
    /// time, nor while the window is in the background.
    pub fn should_fetch(&self, now_ms: u64, focused: bool) -> bool {
        focused && !self.asking && self.next_fetch_ms.is_none_or(|at| now_ms >= at)
    }

    /// A request goes out; the token to send with it.
    pub fn asked(&mut self) -> Option<String> {
        self.asking = true;
        self.refresh_token.clone()
    }

    pub fn loaded(&mut self, feed: Feed, now_ms: u64) {
        self.asking = false;
        self.failures = 0;
        self.entries = feed.entries.into_iter().map(|e| e.content).collect();
        if feed.refresh_token.is_some() {
            self.refresh_token = feed.refresh_token;
        }
        self.next_fetch_ms = Some(now_ms + feed.wait_ms_until_next_fetch.unwrap_or(DEFAULT_WAIT_MS));
    }

    /// A failed request: try again after 1, 2, 4 then 8 minutes; past
    /// that the feed is emptied and the last wait repeats.
    pub fn failed(&mut self, now_ms: u64) {
        self.asking = false;
        let wait = BACKOFF_MS[self.failures.min(BACKOFF_MS.len() - 1)];
        self.failures += 1;
        if self.failures > BACKOFF_MS.len() {
            self.entries.clear();
        }
        self.next_fetch_ms = Some(now_ms + wait);
    }

    /// Discord said the feed went stale: due again after `after_ms`.
    pub fn stale(&mut self, after_ms: u64, now_ms: u64) {
        let at = now_ms + after_ms;
        self.next_fetch_ms = Some(self.next_fetch_ms.map_or(at, |next| next.min(at)));
    }

    /// When the next request is due.
    #[cfg(test)]
    pub fn next_fetch_ms(&self) -> Option<u64> {
        self.next_fetch_ms
    }

    /// The entries a server's member list shows: the kinds it draws, by
    /// authors `visible` accepts (members who can see the channel).
    pub fn for_guild(&self, visible: impl Fn(Snowflake) -> bool) -> Vec<&Content> {
        self.entries
            .iter()
            .filter(|c| SHOWN.contains(&c.content_type) && visible(c.author_id))
            .collect()
    }
}

/// Whether a server shows the Activity section: its own setting when it
/// has one, else only under 10,000 members.
pub fn shown_in(features: &[String], member_count: u64) -> bool {
    if features.iter().any(|f| f == "ACTIVITY_FEED_ENABLED_BY_USER") {
        return true;
    }
    if features.iter().any(|f| f == "ACTIVITY_FEED_DISABLED_BY_USER") {
        return false;
    }
    member_count < 10_000
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(entries: &str, wait: u64) -> Feed {
        serde_json::from_str(&format!(r#"{{"entries":[{entries}],"refresh_token":"r","wait_ms_until_next_fetch":{wait}}}"#))
            .unwrap()
    }

    fn entry(id: &str, author: u64, kind: u32) -> String {
        format!(r#"{{"content":{{"id":"{id}","author_id":"{author}","content_type":{kind}}}}}"#)
    }

    #[test]
    fn waits_as_told_and_sends_the_token_back() {
        let mut feed_store = ActivityFeed::default();
        assert!(feed_store.should_fetch(0, true));
        assert!(!feed_store.should_fetch(0, false));
        assert_eq!(feed_store.asked(), None);
        assert!(!feed_store.should_fetch(0, true), "one request at a time");
        feed_store.loaded(feed("", 30_000), 1_000);
        assert!(!feed_store.should_fetch(30_000, true));
        assert!(feed_store.should_fetch(31_000, true));
        assert_eq!(feed_store.asked().as_deref(), Some("r"));
    }

    #[test]
    fn failures_back_off_then_empty_the_feed() {
        let mut feed_store = ActivityFeed::default();
        feed_store.loaded(feed(&entry("a", 1, 1), 0), 0);
        let mut waits = Vec::new();
        for _ in 0..5 {
            feed_store.asked();
            feed_store.failed(0);
            waits.push(feed_store.next_fetch_ms().unwrap());
        }
        assert_eq!(waits, [60_000, 120_000, 240_000, 480_000, 480_000]);
        assert!(feed_store.for_guild(|_| true).is_empty());
    }

    #[test]
    fn stale_brings_the_next_fetch_forward() {
        let mut feed_store = ActivityFeed::default();
        feed_store.loaded(feed("", 600_000), 0);
        feed_store.stale(5_000, 1_000);
        assert_eq!(feed_store.next_fetch_ms(), Some(6_000));
    }

    #[test]
    fn guilds_show_known_kinds_by_visible_members() {
        let mut feed_store = ActivityFeed::default();
        let entries = [entry("a", 1, 1), entry("b", 2, 1), entry("c", 1, 7), entry("d", 1, 99)].join(",");
        feed_store.loaded(feed(&entries, 0), 0);
        let shown: Vec<&str> = feed_store.for_guild(|author| author == Snowflake(1)).iter().map(|c| c.id.as_str()).collect();
        assert_eq!(shown, ["a"]);
    }

    #[test]
    fn big_servers_hide_it_unless_they_turned_it_on() {
        assert!(shown_in(&[], 9_999));
        assert!(!shown_in(&[], 10_000));
        assert!(shown_in(&["ACTIVITY_FEED_ENABLED_BY_USER".into()], 50_000));
        assert!(!shown_in(&["ACTIVITY_FEED_DISABLED_BY_USER".into()], 5));
    }
}
