//! A small reorder buffer per speaker: packets wait 40 ms before playing,
//! late ones slot back in order, and a short gap is filled by the decoder's
//! loss concealment rather than silence.

/// Packets held at once.
const SLOTS: usize = 8;
/// Ticks to wait after the first packet before playing.
const START_DELAY: u8 = 2;
/// Lost packets in a row concealed before skipping ahead.
const MAX_CONCEALED: u8 = 3;
/// Largest Opus packet.
const MAX_OPUS: usize = 1275;

/// What to play next.
#[derive(Debug, PartialEq, Eq)]
pub enum Next {
    Packet(Vec<u8>),
    /// The packet is missing; conceal it.
    Lost,
    /// Nothing to play.
    Silence,
}

#[derive(Default)]
pub struct Jitter {
    packets: Vec<(u16, Vec<u8>)>,
    next: Option<u16>,
    wait: u8,
    missing: u8,
}

impl Jitter {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn push(&mut self, sequence: u16, opus: Vec<u8>) {
        if opus.len() > MAX_OPUS {
            return;
        }
        let next = *self.next.get_or_insert_with(|| {
            self.wait = START_DELAY;
            sequence
        });
        let ahead = sequence.wrapping_sub(next);
        // Behind what was already played.
        if ahead >= u16::MAX / 2 {
            return;
        }
        // Far ahead: the sender skipped; start over from here.
        if usize::from(ahead) >= SLOTS {
            self.packets.clear();
            self.next = Some(sequence);
            self.wait = START_DELAY;
            self.missing = 0;
        }
        if self.packets.len() < SLOTS && !self.packets.iter().any(|(s, _)| *s == sequence) {
            self.packets.push((sequence, opus));
        }
    }

    pub fn pop(&mut self) -> Next {
        let Some(next) = self.next else { return Next::Silence };
        if self.wait > 0 && self.packets.len() < SLOTS {
            self.wait -= 1;
            return Next::Silence;
        }
        self.wait = 0;
        self.next = Some(next.wrapping_add(1));
        if let Some(ix) = self.packets.iter().position(|(s, _)| *s == next) {
            self.missing = 0;
            return Next::Packet(self.packets.swap_remove(ix).1);
        }
        self.missing += 1;
        if self.missing <= MAX_CONCEALED {
            return Next::Lost;
        }
        // A long gap: play the oldest packet that did arrive, or rest.
        match self.packets.iter().enumerate().min_by_key(|(_, (s, _))| s.wrapping_sub(next)) {
            Some((ix, (sequence, _))) => {
                self.next = Some(sequence.wrapping_add(1));
                self.missing = 0;
                Next::Packet(self.packets.swap_remove(ix).1)
            }
            None => {
                self.clear();
                Next::Silence
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reorders_after_the_start_delay() {
        let mut jitter = Jitter::default();
        jitter.push(u16::MAX, vec![1]);
        jitter.push(1, vec![3]);
        jitter.push(0, vec![2]);
        jitter.push(0, vec![9]);
        assert_eq!(jitter.pop(), Next::Silence);
        assert_eq!(jitter.pop(), Next::Silence);
        assert_eq!(jitter.pop(), Next::Packet(vec![1]));
        assert_eq!(jitter.pop(), Next::Packet(vec![2]));
        assert_eq!(jitter.pop(), Next::Packet(vec![3]));
    }

    #[test]
    fn short_gaps_are_concealed_and_long_ones_skipped() {
        let mut jitter = Jitter::default();
        jitter.push(10, vec![0]);
        jitter.push(16, vec![6]);
        for _ in 0..START_DELAY {
            assert_eq!(jitter.pop(), Next::Silence);
        }
        assert_eq!(jitter.pop(), Next::Packet(vec![0]));
        for _ in 0..MAX_CONCEALED {
            assert_eq!(jitter.pop(), Next::Lost);
        }
        assert_eq!(jitter.pop(), Next::Packet(vec![6]));
        assert_eq!(jitter.pop(), Next::Lost);
    }

    #[test]
    fn stays_bounded() {
        let mut jitter = Jitter::default();
        for sequence in 0..1000 {
            jitter.push(sequence, vec![1; MAX_OPUS]);
            assert!(jitter.packets.len() <= SLOTS);
        }
        jitter.push(1000, vec![0; MAX_OPUS + 1]);
        assert!(jitter.packets.len() <= SLOTS);
    }
}
