//! Images by URL under a cap on how many and on the bytes their pixels
//! take. The oldest go first, except ones drawn in the last two frames, so
//! a screenful is never evicted while on screen (and never thrashes).

use std::collections::{HashMap, VecDeque};

use gpui::SharedString;

pub(super) struct Budget<T> {
    slots: HashMap<SharedString, Slot<T>>,
    /// URLs oldest first; ones drawn lately go back to the end when their
    /// turn to be evicted comes.
    order: VecDeque<SharedString>,
    /// What the values in `slots` take decoded.
    bytes: usize,
    max_count: usize,
    max_bytes: usize,
}

struct Slot<T> {
    /// `None` while on its way or after a failure, so each URL is asked
    /// for once.
    value: Option<T>,
    bytes: usize,
    /// The frame it was last drawn in.
    drawn: u64,
}

impl<T> Budget<T> {
    pub fn new(max_count: usize, max_bytes: usize) -> Self {
        Self {
            slots: HashMap::new(),
            order: VecDeque::new(),
            bytes: 0,
            max_count,
            max_bytes,
        }
    }

    /// The slot for `url` drawn in `frame`: `None` when unknown, so the
    /// caller asks for it with [`Budget::ask`].
    pub fn get(&mut self, url: &str, frame: u64) -> Option<Option<&T>> {
        let slot = self.slots.get_mut(url)?;
        slot.drawn = frame;
        Some(slot.value.as_ref())
    }

    /// Makes room for `url` and holds its place until [`Budget::fill`];
    /// returns what had to go.
    pub fn ask(&mut self, url: SharedString, frame: u64) -> Vec<T> {
        let evicted = self.evict(1, frame);
        self.order.push_back(url.clone());
        self.slots.insert(url, Slot { value: None, bytes: 0, drawn: frame });
        evicted
    }

    /// `value` arrived for `url`, taking `bytes`; returns what had to go,
    /// or `value` itself when `url` was evicted while on its way.
    pub fn fill(&mut self, url: &str, value: T, bytes: usize, frame: u64) -> Vec<T> {
        let Some(slot) = self.slots.get_mut(url) else { return vec![value] };
        self.bytes = self.bytes - slot.bytes + bytes;
        slot.bytes = bytes;
        let mut evicted: Vec<T> = slot.value.replace(value).into_iter().collect();
        evicted.extend(self.evict(0, frame));
        evicted
    }

    /// Drops the oldest values not drawn lately until `incoming` more fit
    /// under both caps.
    fn evict(&mut self, incoming: usize, frame: u64) -> Vec<T> {
        let recent = frame.saturating_sub(1);
        let mut evicted = Vec::new();
        let mut spared = 0;
        while (self.order.len() + incoming > self.max_count || self.bytes > self.max_bytes) && spared < self.order.len() {
            let Some(oldest) = self.order.pop_front() else { break };
            if self.slots.get(&oldest).is_some_and(|slot| slot.drawn >= recent) {
                self.order.push_back(oldest);
                spared += 1;
            } else if let Some(slot) = self.slots.remove(&oldest) {
                self.bytes -= slot.bytes;
                evicted.extend(slot.value);
            }
        }
        evicted
    }

    /// Whether `url` was asked for and not evicted since.
    pub fn contains(&self, url: &str) -> bool {
        self.slots.contains_key(url)
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    #[cfg(test)]
    pub fn bytes(&self) -> usize {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oldest_go_first_unless_drawn_lately() {
        let mut budget = Budget::new(3, usize::MAX);
        for url in ["a", "b", "c"] {
            budget.ask(url.into(), 0);
            budget.fill(url, url, 1, 0);
        }
        assert!(budget.get("a", 2).is_some());
        assert_eq!(budget.ask("d".into(), 2), vec!["b"]);
        assert!(budget.contains("a") && !budget.contains("b"));
    }

    #[test]
    fn bytes_are_capped_but_a_screenful_stays() {
        let mut budget = Budget::new(10, 100);
        budget.ask("big".into(), 0);
        // Over the cap but drawn this frame: kept.
        assert!(budget.fill("big", 1, 150, 0).is_empty());
        assert_eq!(budget.ask("small".into(), 2), vec![1]);
        assert!(budget.fill("small", 2, 10, 2).is_empty());
        assert_eq!(budget.bytes(), 10);
        assert_eq!(budget.len(), 1);
    }

    #[test]
    fn late_arrivals_for_evicted_urls_come_back() {
        let mut budget = Budget::new(1, usize::MAX);
        budget.ask("a".into(), 0);
        budget.ask("b".into(), 5);
        assert_eq!(budget.fill("a", 7, 1, 5), vec![7]);
    }
}
