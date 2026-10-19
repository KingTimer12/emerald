//! Time-based containers the stores share. They take `now` from the caller
//! so tests can drive the clock.

use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};

/// Values fetched on demand and refetched once older than `ttl`. A stale
/// value stays readable until its replacement arrives.
pub struct TtlCache<K, V> {
    ttl: Duration,
    entries: HashMap<K, (Instant, Option<V>)>,
}

impl<K: Eq + Hash, V> TtlCache<K, V> {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            entries: HashMap::new(),
        }
    }

    /// Whether `key` should be fetched now; if so, records the request so
    /// it is not repeated before `ttl` passes.
    pub fn request(&mut self, key: K, now: Instant) -> bool {
        match self.entries.get_mut(&key) {
            Some((requested, _)) if now.duration_since(*requested) < self.ttl => false,
            Some((requested, _)) => {
                *requested = now;
                true
            }
            None => {
                self.entries.insert(key, (now, None));
                true
            }
        }
    }

    pub fn fill(&mut self, key: K, value: V, now: Instant) {
        self.entries.entry(key).or_insert((now, None)).1 = Some(value);
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.entries.get(key)?.1.as_ref()
    }
}

/// Keys that each lapse at their own deadline, in the order they were set.
pub struct Deadlines<T> {
    entries: Vec<(T, Instant)>,
}

impl<T> Default for Deadlines<T> {
    fn default() -> Self {
        Self { entries: Vec::new() }
    }
}

impl<T: PartialEq> Deadlines<T> {
    /// Sets or moves `key`'s deadline, making it the most recent.
    pub fn set(&mut self, key: T, until: Instant) {
        self.remove(&key);
        self.entries.push((key, until));
    }

    pub fn remove(&mut self, key: &T) {
        self.entries.retain(|(k, _)| k != key);
    }

    pub fn prune(&mut self, now: Instant) {
        self.entries.retain(|(_, until)| *until > now);
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.entries.iter().map(|(key, _)| key)
    }
}

/// Lets an action through at most once per `interval` for the same key.
pub struct Throttle<K> {
    interval: Duration,
    last: Option<(K, Instant)>,
}

impl<K: PartialEq + Copy> Throttle<K> {
    pub fn new(interval: Duration) -> Self {
        Self { interval, last: None }
    }

    /// Whether the action for `key` may run now; if so, starts its interval.
    pub fn allow(&mut self, key: K, now: Instant) -> bool {
        let recent = self
            .last
            .is_some_and(|(last, at)| last == key && now.duration_since(at) < self.interval);
        if !recent {
            self.last = Some((key, now));
        }
        !recent
    }

    pub fn reset(&mut self) {
        self.last = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECOND: Duration = Duration::from_secs(1);

    #[test]
    fn ttl_cache_refetches_only_when_stale() {
        let start = Instant::now();
        let mut cache = TtlCache::new(10 * SECOND);
        assert!(cache.request(1, start));
        assert!(!cache.request(1, start + 5 * SECOND));
        cache.fill(1, "old", start);
        assert!(cache.request(1, start + 11 * SECOND));
        // The stale value stays until the new one lands.
        assert_eq!(cache.get(&1), Some(&"old"));
    }

    #[test]
    fn deadlines_lapse_and_reorder() {
        let start = Instant::now();
        let mut set = Deadlines::default();
        set.set("a", start + SECOND);
        set.set("b", start + 3 * SECOND);
        set.set("a", start + 2 * SECOND);
        assert_eq!(set.iter().collect::<Vec<_>>(), [&"b", &"a"]);
        set.prune(start + 2 * SECOND);
        assert_eq!(set.iter().collect::<Vec<_>>(), [&"b"]);
    }

    #[test]
    fn throttle_is_per_key() {
        let start = Instant::now();
        let mut throttle = Throttle::new(8 * SECOND);
        assert!(throttle.allow(1, start));
        assert!(!throttle.allow(1, start + SECOND));
        assert!(throttle.allow(2, start + SECOND));
        assert!(throttle.allow(2, start + 9 * SECOND));
        throttle.reset();
        assert!(throttle.allow(2, start + 9 * SECOND));
    }
}
