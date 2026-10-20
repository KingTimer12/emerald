//! Read state per channel: what was read, what is unread, the mentions
//! waiting to be seen, and the read receipts waiting to go out.

use std::collections::{HashMap, HashSet};

use crate::discord::model::{Channel, Snowflake};

#[derive(Default)]
pub struct ReadStates {
    /// Last read message per channel.
    read: HashMap<Snowflake, Snowflake>,
    unread: HashSet<Snowflake>,
    /// Unread mentions per channel; in DMs every message counts.
    mentions: HashMap<Snowflake, u32>,
    /// Channels with a receipt scheduled, so bursts send one.
    scheduled: HashSet<Snowflake>,
}

impl ReadStates {
    /// A new session's read states; `channels` are those the user can read,
    /// unread when their newest message is past the read one.
    /// Each entry of `read` is a channel, its last read message and the
    /// mentions since.
    pub fn load<'a>(&mut self, read: Vec<(Snowflake, Snowflake, u32)>, channels: impl Iterator<Item = &'a Channel>) {
        self.mentions = read.iter().filter(|r| r.2 > 0).map(|r| (r.0, r.2)).collect();
        self.read = read.into_iter().map(|(channel, message, _)| (channel, message)).collect();
        self.unread = channels
            .filter(|c| match (c.last_message_id, self.read.get(&c.id)) {
                (Some(last), Some(read)) => last > *read,
                _ => false,
            })
            .map(|c| c.id)
            .collect();
    }

    pub fn is_unread(&self, channel: Snowflake) -> bool {
        self.unread.contains(&channel)
    }

    pub fn mark_unread(&mut self, channel: Snowflake) {
        self.unread.insert(channel);
    }

    pub fn mentions(&self, channel: Snowflake) -> u32 {
        self.mentions.get(&channel).copied().unwrap_or(0)
    }

    /// Unread mentions across `channels`.
    pub fn total_mentions<'a>(&self, channels: impl Iterator<Item = &'a Channel>) -> u32 {
        channels.map(|c| self.mentions(c.id)).sum()
    }

    /// A message pinging the user arrived off screen.
    pub fn add_mention(&mut self, channel: Snowflake) {
        *self.mentions.entry(channel).or_default() += 1;
        self.unread.insert(channel);
    }

    /// The channel is on screen.
    pub fn seen(&mut self, channel: Snowflake) {
        self.unread.remove(&channel);
        self.mentions.remove(&channel);
    }

    /// MESSAGE_ACK: read here or on another device, with the mentions it
    /// leaves unread.
    pub fn acked(&mut self, channel: Snowflake, message: Snowflake, mentions: u32) {
        self.read.insert(channel, message);
        self.unread.remove(&channel);
        if mentions > 0 {
            self.mentions.insert(channel, mentions);
            self.unread.insert(channel);
        } else {
            self.mentions.remove(&channel);
        }
    }

    pub fn has_read(&self, channel: Snowflake, message: Snowflake) -> bool {
        self.read.get(&channel).is_some_and(|read| *read >= message)
    }

    /// Opens the window that collects a burst into one receipt; false when
    /// one is already open for `channel` or `message` is already read.
    pub fn schedule(&mut self, channel: Snowflake, message: Snowflake) -> bool {
        !self.has_read(channel, message) && self.scheduled.insert(channel)
    }

    /// Closes the window: the message to send a receipt for, if `newest`
    /// is still unread, now recorded as read.
    pub fn flush(&mut self, channel: Snowflake, newest: Option<Snowflake>) -> Option<Snowflake> {
        self.scheduled.remove(&channel);
        let newest = newest.filter(|m| !self.has_read(channel, *m))?;
        self.read.insert(channel, newest);
        Some(newest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const C: Snowflake = Snowflake(1);

    #[test]
    fn unread_from_read_states() {
        let channel = |id, last| Channel {
            id: Snowflake(id),
            last_message_id: Some(Snowflake(last)),
            ..Channel::default()
        };
        let channels = [channel(1, 10), channel(2, 20), channel(3, 30)];
        let mut reads = ReadStates::default();
        reads.load(vec![(Snowflake(1), Snowflake(10), 0), (Snowflake(2), Snowflake(5), 2)], channels.iter());
        assert!(!reads.is_unread(Snowflake(1)));
        assert!(reads.is_unread(Snowflake(2)));
        // Never read: Discord does not mark it.
        assert!(!reads.is_unread(Snowflake(3)));
        assert_eq!(reads.total_mentions(channels.iter()), 2);
    }

    #[test]
    fn mentions_clear_when_seen_or_acked() {
        let mut reads = ReadStates::default();
        reads.add_mention(C);
        reads.add_mention(C);
        assert_eq!(reads.mentions(C), 2);
        assert!(reads.is_unread(C));
        reads.seen(C);
        assert_eq!(reads.mentions(C), 0);
        reads.add_mention(C);
        reads.acked(C, Snowflake(3), 0);
        assert_eq!(reads.mentions(C), 0);
        // "Mark unread" on another device brings them back.
        reads.acked(C, Snowflake(3), 1);
        assert_eq!(reads.mentions(C), 1);
    }

    #[test]
    fn bursts_collapse_into_one_receipt() {
        let mut reads = ReadStates::default();
        assert!(reads.schedule(C, Snowflake(5)));
        assert!(!reads.schedule(C, Snowflake(6)));
        assert_eq!(reads.flush(C, Some(Snowflake(7))), Some(Snowflake(7)));
        assert!(!reads.schedule(C, Snowflake(7)));
        assert!(reads.schedule(C, Snowflake(8)));
        reads.acked(C, Snowflake(9), 0);
        assert_eq!(reads.flush(C, Some(Snowflake(8))), None);
    }
}
