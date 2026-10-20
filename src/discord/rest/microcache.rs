//! Microcaching, after nginx's
//! (<https://blog.nginx.org/blog/benefits-of-microcaching-nginx>): GET
//! answers are kept for about a second, so a burst of identical requests
//! (two views asking for the same profile, a reconnect replaying fetches,
//! clicking back and forth between channels) costs Discord one request.
//! It follows the three rules the article pairs with the short validity:
//!
//! - a lock (`proxy_cache_lock`): while a request for a key is in flight,
//!   the others wait for its answer instead of sending their own;
//! - stale while updating (`proxy_cache_use_stale updating`): while a
//!   refresh is in flight, an expired copy still within `Policy::stale` is
//!   served at once;
//! - stale on error (`proxy_cache_use_stale error timeout http_5xx`): when
//!   Discord cannot answer, a copy within `Policy::stale` stands in. `Rest`
//!   applies that one, since only it knows which failures are transient.
//!
//! A write to a channel or guild drops the copies scoped to it.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use bytes::Bytes;
use tokio::sync::watch;
use tokio::time::Instant;

use crate::discord::model::Snowflake;

/// Entries kept before expired ones are swept out.
const SWEEP_ABOVE: usize = 256;

/// How long a route's answers may be served from the cache.
#[derive(Clone, Copy, Debug)]
pub struct Policy {
    /// Served without asking Discord.
    pub fresh: Duration,
    /// Served while a refresh is in flight, or when the refresh fails.
    pub stale: Duration,
    /// Also kept on disk (`cache::Cache::put_response`), for errors after a
    /// restart.
    pub persist: bool,
}

impl Policy {
    /// The microcache proper: one second, as in the article.
    pub const MICRO: Self = Self {
        fresh: Duration::from_secs(1),
        stale: Duration::from_secs(10),
        persist: false,
    };
}

/// What the request in flight answered: the body, or the error's message.
pub type Answer = Option<Result<Bytes, String>>;

struct Entry {
    body: Option<Bytes>,
    fetched: Instant,
    stale: Duration,
    scope: Snowflake,
    /// Set while a request for this key is in flight.
    inflight: Option<watch::Receiver<Answer>>,
}

#[derive(Default)]
pub struct MicroCache {
    entries: Mutex<HashMap<String, Entry>>,
}

pub enum Lookup<'a> {
    Hit(Bytes),
    /// Another request for the key is in flight. If it goes away without
    /// an answer (cancelled), look up again.
    Wait(watch::Receiver<Answer>),
    /// Send the request and hand the answer to the ticket.
    Fetch(Ticket<'a>),
}

impl MicroCache {
    pub fn lookup(&self, key: &str, policy: Policy, scope: Snowflake) -> Lookup<'_> {
        let now = Instant::now();
        let mut entries = self.entries.lock().unwrap();
        if let Some(entry) = entries.get_mut(key) {
            let age = now.saturating_duration_since(entry.fetched);
            if let Some(body) = &entry.body {
                let usable = if entry.inflight.is_some() { policy.stale } else { policy.fresh };
                if age < usable {
                    return Lookup::Hit(body.clone());
                }
            }
            if let Some(inflight) = &entry.inflight {
                return Lookup::Wait(inflight.clone());
            }
        }
        let (tx, rx) = watch::channel(None);
        let entry = entries.entry(key.to_owned()).or_insert(Entry {
            body: None,
            fetched: now,
            stale: policy.stale,
            scope,
            inflight: None,
        });
        entry.inflight = Some(rx);
        entry.stale = policy.stale;
        let stale = entry
            .body
            .clone()
            .filter(|_| now.saturating_duration_since(entry.fetched) < policy.stale);
        Lookup::Fetch(Ticket {
            cache: self,
            key: key.to_owned(),
            tx,
            stale,
        })
    }

    /// Drops what is cached for `scope`, after a write to it.
    pub fn invalidate(&self, scope: Snowflake) {
        let mut entries = self.entries.lock().unwrap();
        for entry in entries.values_mut().filter(|e| e.scope == scope) {
            entry.body = None;
        }
    }

    fn store(&self, key: &str, body: Option<Bytes>) {
        let now = Instant::now();
        let mut entries = self.entries.lock().unwrap();
        if let Some(entry) = entries.get_mut(key) {
            entry.inflight = None;
            if let Some(body) = body {
                entry.body = Some(body);
                entry.fetched = now;
            }
        }
        if entries.len() > SWEEP_ABOVE {
            entries.retain(|_, e| {
                e.inflight.is_some()
                    || (e.body.is_some() && now.saturating_duration_since(e.fetched) < e.stale)
            });
        }
    }
}

/// The right to fetch a key. Dropping it without an answer (the request
/// was cancelled) releases the waiters, which then try on their own.
pub struct Ticket<'a> {
    cache: &'a MicroCache,
    key: String,
    tx: watch::Sender<Answer>,
    stale: Option<Bytes>,
}

impl Ticket<'_> {
    pub fn key(&self) -> &str {
        &self.key
    }

    /// The expired copy still within the route's stale window, if any.
    pub fn stale(&self) -> Option<&Bytes> {
        self.stale.as_ref()
    }

    /// Caches `body` and hands it to the waiters.
    pub fn finish(self, body: Bytes) {
        self.cache.store(&self.key, Some(body.clone()));
        self.tx.send_replace(Some(Ok(body)));
    }

    /// Hands the waiters an answer that is not to be cached: a stale copy
    /// standing in for a failure, or the failure itself.
    pub fn share(self, answer: Result<Bytes, String>) {
        self.cache.store(&self.key, None);
        self.tx.send_replace(Some(answer));
    }
}

impl Drop for Ticket<'_> {
    fn drop(&mut self) {
        if self.tx.borrow().is_none() {
            self.cache.store(&self.key, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCOPE: Snowflake = Snowflake(7);

    fn fetch<'a>(cache: &'a MicroCache, key: &str) -> Ticket<'a> {
        match cache.lookup(key, Policy::MICRO, SCOPE) {
            Lookup::Fetch(ticket) => ticket,
            _ => panic!("expected a fetch"),
        }
    }

    fn hit(cache: &MicroCache, key: &str) -> Bytes {
        match cache.lookup(key, Policy::MICRO, SCOPE) {
            Lookup::Hit(body) => body,
            _ => panic!("expected a hit"),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn fresh_for_a_second() {
        let cache = MicroCache::default();
        fetch(&cache, "/a").finish(Bytes::from_static(b"1"));
        assert_eq!(hit(&cache, "/a"), "1");
        tokio::time::advance(Duration::from_millis(1100)).await;
        let ticket = fetch(&cache, "/a");
        assert_eq!(ticket.stale().map(|b| &b[..]), Some(&b"1"[..]));
    }

    #[tokio::test(start_paused = true)]
    async fn concurrent_requests_wait_for_the_first() {
        let cache = MicroCache::default();
        let ticket = fetch(&cache, "/a");
        let Lookup::Wait(mut rx) = cache.lookup("/a", Policy::MICRO, SCOPE) else {
            panic!("expected to wait");
        };
        ticket.finish(Bytes::from_static(b"1"));
        let answer = rx.wait_for(Option::is_some).await.unwrap().clone();
        assert_eq!(answer, Some(Ok(Bytes::from_static(b"1"))));
    }

    #[tokio::test(start_paused = true)]
    async fn stale_copy_served_while_updating() {
        let cache = MicroCache::default();
        fetch(&cache, "/a").finish(Bytes::from_static(b"old"));
        tokio::time::advance(Duration::from_secs(2)).await;
        let refresh = fetch(&cache, "/a");
        assert_eq!(hit(&cache, "/a"), "old");
        refresh.finish(Bytes::from_static(b"new"));
        assert_eq!(hit(&cache, "/a"), "new");
    }

    #[tokio::test(start_paused = true)]
    async fn waiters_share_a_failure_without_caching_it() {
        let cache = MicroCache::default();
        let ticket = fetch(&cache, "/a");
        let Lookup::Wait(mut rx) = cache.lookup("/a", Policy::MICRO, SCOPE) else {
            panic!("expected to wait");
        };
        ticket.share(Err("Unknown User".into()));
        let answer = rx.wait_for(Option::is_some).await.unwrap().clone();
        assert_eq!(answer, Some(Err("Unknown User".into())));
        fetch(&cache, "/a");
    }

    #[tokio::test(start_paused = true)]
    async fn cancelled_fetch_releases_waiters() {
        let cache = MicroCache::default();
        let ticket = fetch(&cache, "/a");
        let Lookup::Wait(mut rx) = cache.lookup("/a", Policy::MICRO, SCOPE) else {
            panic!("expected to wait");
        };
        drop(ticket);
        assert!(rx.wait_for(Option::is_some).await.is_err());
        // The next caller fetches on its own.
        fetch(&cache, "/a");
    }

    #[tokio::test(start_paused = true)]
    async fn writes_drop_the_scope() {
        let cache = MicroCache::default();
        fetch(&cache, "/a").finish(Bytes::from_static(b"1"));
        cache.invalidate(Snowflake(8));
        hit(&cache, "/a");
        cache.invalidate(SCOPE);
        assert!(fetch(&cache, "/a").stale().is_none());
    }
}
