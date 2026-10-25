//! Reactions and poll votes on a message, counted as gateway events arrive.

use super::*;

/// Everyone's reactions with one emoji. Super reactions are counted apart
/// and drawn as their own chip.
#[derive(Clone, Debug, PartialEq)]
pub struct Reacted {
    pub emoji: Emoji,
    pub normal: u32,
    pub burst: u32,
    /// We reacted with it.
    pub me: bool,
    pub me_burst: bool,
}

impl From<Reaction> for Reacted {
    fn from(reaction: Reaction) -> Self {
        let counts = reaction.counts();
        Self {
            emoji: reaction.emoji,
            normal: counts.normal,
            burst: counts.burst,
            me: reaction.me,
            me_burst: reaction.me_burst,
        }
    }
}

impl Msg {
    /// One user's reaction came or went; `mine` when that user is us.
    /// Repeats of our own (the gateway echoing what we already show) change
    /// nothing. True when something changed.
    pub fn react(&mut self, emoji: &Emoji, burst: bool, mine: bool, add: bool) -> bool {
        let found = self.reactions.iter().position(|r| r.emoji.same(emoji));
        let Some(ix) = found else {
            if add {
                self.reactions.push(Reacted {
                    emoji: emoji.clone(),
                    normal: u32::from(!burst),
                    burst: u32::from(burst),
                    me: mine && !burst,
                    me_burst: mine && burst,
                });
            }
            return add;
        };
        let reaction = &mut self.reactions[ix];
        let (count, me) = if burst {
            (&mut reaction.burst, &mut reaction.me_burst)
        } else {
            (&mut reaction.normal, &mut reaction.me)
        };
        if mine && *me == add {
            return false;
        }
        if mine {
            *me = add;
        }
        *count = if add { *count + 1 } else { count.saturating_sub(1) };
        if reaction.normal == 0 && reaction.burst == 0 {
            self.reactions.remove(ix);
        }
        true
    }

    /// Drops the reactions with `emoji`, or all of them.
    pub fn clear_reactions(&mut self, emoji: Option<&Emoji>) -> bool {
        let before = self.reactions.len();
        match emoji {
            Some(emoji) => self.reactions.retain(|r| !r.emoji.same(emoji)),
            None => self.reactions.clear(),
        }
        self.reactions.len() != before
    }

    /// One user's vote on a poll answer came or went. Our own is marked
    /// too; its echo of what we already show changes nothing.
    pub fn poll_vote(&mut self, answer: u32, mine: bool, add: bool) -> bool {
        let Some(poll) = &mut self.poll else { return false };
        if !poll.answers.iter().any(|a| a.answer_id == answer) {
            return false;
        }
        let results = poll.results.get_or_insert_with(PollResults::default);
        if results.is_finalized {
            return false;
        }
        let ix = match results.answer_counts.iter().position(|c| c.id == answer) {
            Some(ix) => ix,
            None => {
                results.answer_counts.push(PollAnswerCount {
                    id: answer,
                    count: 0,
                    me_voted: false,
                });
                results.answer_counts.len() - 1
            }
        };
        let count = &mut results.answer_counts[ix];
        if mine {
            if count.me_voted == add {
                return false;
            }
            count.me_voted = add;
        }
        count.count = if add { count.count + 1 } else { count.count.saturating_sub(1) };
        true
    }
}
