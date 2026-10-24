//! What was being written in each channel, kept while the user looks
//! elsewhere and put back when they return, as the web client does.

use std::collections::VecDeque;

use crate::discord::model::Snowflake;

/// Drafts kept at most; the channel left longest ago loses its own first.
const CAPACITY: usize = 50;

/// Drafts by channel, newest last. `D` is whatever the message box holds:
/// text, the reply and the files waiting to go.
pub struct Drafts<D> {
    drafts: VecDeque<(Snowflake, D)>,
    capacity: usize,
}

impl<D> Default for Drafts<D> {
    fn default() -> Self {
        Self::with_capacity(CAPACITY)
    }
}

impl<D> Drafts<D> {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            drafts: VecDeque::new(),
            capacity: capacity.max(1),
        }
    }

    /// Keeps `draft` for `channel`, replacing an older one.
    pub fn keep(&mut self, channel: Snowflake, draft: D) {
        self.take(channel);
        if self.drafts.len() == self.capacity {
            self.drafts.pop_front();
        }
        self.drafts.push_back((channel, draft));
    }

    /// The draft for `channel`, which is then no longer kept.
    pub fn take(&mut self, channel: Snowflake) -> Option<D> {
        let ix = self.drafts.iter().position(|(id, _)| *id == channel)?;
        self.drafts.remove(ix).map(|(_, draft)| draft)
    }

    #[cfg(test)]
    pub fn has(&self, channel: Snowflake) -> bool {
        self.drafts.iter().any(|(id, _)| *id == channel)
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.drafts.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drafts_come_back_once() {
        let mut drafts = Drafts::default();
        drafts.keep(Snowflake(1), "hello");
        drafts.keep(Snowflake(2), "other");
        drafts.keep(Snowflake(1), "hello again");
        assert_eq!(drafts.len(), 2);
        assert_eq!(drafts.take(Snowflake(1)), Some("hello again"));
        assert_eq!(drafts.take(Snowflake(1)), None);
        assert!(drafts.has(Snowflake(2)));
    }

    #[test]
    fn the_oldest_draft_goes_first() {
        let mut drafts = Drafts::with_capacity(2);
        drafts.keep(Snowflake(1), 1);
        drafts.keep(Snowflake(2), 2);
        // Touching 1 makes 2 the oldest.
        drafts.keep(Snowflake(1), 10);
        drafts.keep(Snowflake(3), 3);
        assert!(!drafts.has(Snowflake(2)));
        assert_eq!(drafts.take(Snowflake(1)), Some(10));
        assert_eq!(drafts.take(Snowflake(3)), Some(3));
    }
}
