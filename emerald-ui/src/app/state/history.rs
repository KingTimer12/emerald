//! Message history of the channels viewed recently, and the contract that
//! keeps the on-screen list in step with it.

mod paging;

use std::collections::{HashMap, HashSet, VecDeque};

use super::msg::{Msg, mention_names};
use emerald_discord::Page;
use emerald_discord::model::{Message, MessageDelete, MessageUpdate, Snowflake};
use emerald_discord::rest::HISTORY_PAGE;

/// Channels whose history stays in memory; the least recently viewed are evicted.
const CACHED_CHANNELS: usize = 8;
/// Messages kept per cached channel; older ones are dropped and refetched on demand.
pub const MESSAGES_PER_CHANNEL: usize = 300;

/// The list showing the open channel. Positions are message indices in its
/// log; the view adds whatever rows it draws above them.
pub trait MessageView {
    /// The log was replaced and holds `messages` now.
    fn reset(&self, messages: usize);
    fn insert(&self, at: usize, count: usize);
    fn remove(&self, at: usize);
    /// The message at `at` changed in place.
    fn refresh(&self, at: usize);
    /// `count` older messages went on top; what is on screen stays put.
    fn prepend(&self, count: usize);
    /// Whether the history is complete changed, and with it the intro row.
    fn refresh_intro(&self);
}

pub struct Log {
    pub messages: VecDeque<Msg>,
    /// The log reaches the channel's first message.
    pub complete: bool,
    /// The log reaches the channel's newest message. False after a jump
    /// to an older one, until paging forward or back to the present.
    pub latest: bool,
    pub loading: bool,
}

impl Default for Log {
    fn default() -> Self {
        Self {
            messages: VecDeque::new(),
            complete: false,
            latest: true,
            loading: false,
        }
    }
}

impl Log {
    fn position(&self, id: Snowflake) -> Option<usize> {
        self.messages.iter().position(|m| m.id == id)
    }

    fn pending(&self, nonce: &str) -> Option<usize> {
        self.messages.iter().position(|m| m.is_pending(nonce))
    }

    /// Appends `msg`, dropping the oldest message past the cap.
    fn push(&mut self, msg: Msg, view: Option<&dyn MessageView>) {
        let end = self.messages.len();
        self.messages.push_back(msg);
        if let Some(v) = view {
            v.insert(end, 1);
        }
        self.trim_front(view);
    }

    /// Drops the oldest messages past the cap.
    fn trim_front(&mut self, view: Option<&dyn MessageView>) {
        while self.messages.len() > MESSAGES_PER_CHANNEL {
            self.messages.pop_front();
            if let Some(view) = view {
                view.remove(0);
                // No longer at the start: the intro goes away.
                if self.complete {
                    view.refresh_intro();
                }
            }
            self.complete = false;
        }
    }
}

#[derive(Default)]
pub struct History {
    logs: HashMap<Snowflake, Log>,
    /// Cached channels, least recently viewed first.
    recent: VecDeque<Snowflake>,
    /// Channels whose history the server refused.
    forbidden: HashSet<Snowflake>,
}

impl History {
    pub fn get(&self, channel: Snowflake) -> Option<&Log> {
        self.logs.get(&channel)
    }

    pub fn is_forbidden(&self, channel: Snowflake) -> bool {
        self.forbidden.contains(&channel)
    }

    /// Drops every log; a new session may have missed messages.
    pub fn clear(&mut self) {
        self.logs.clear();
        self.recent.clear();
    }

    /// Shows `channel` in `view`, evicting the least recently viewed log
    /// past the cap. True when the caller must fetch its first page, which
    /// happens only for a new log with `readable` history.
    pub fn open(&mut self, channel: Snowflake, readable: bool, view: &dyn MessageView) -> bool {
        self.recent.retain(|c| *c != channel);
        self.recent.push_back(channel);
        while self.recent.len() > CACHED_CHANNELS {
            if let Some(evicted) = self.recent.pop_front() {
                self.logs.remove(&evicted);
            }
        }
        if let Some(log) = self.logs.get(&channel) {
            view.reset(log.messages.len());
            return false;
        }
        // Without history permission only new messages show up.
        let fetch = readable && !self.forbidden.contains(&channel);
        self.logs.insert(
            channel,
            Log {
                loading: fetch,
                complete: !fetch,
                ..Log::default()
            },
        );
        view.reset(0);
        fetch
    }

    /// A message from the gateway, or the REST echo of our own send. A
    /// log away from the present leaves it for paging forward to bring.
    pub fn receive(&mut self, message: Message, mine: bool, view: Option<&dyn MessageView>) {
        let Some(log) = self.logs.get_mut(&message.channel_id) else { return };
        if !log.latest || log.position(message.id).is_some() {
            return;
        }
        // Our own send coming back: confirm the pending copy in place.
        let pending = message.nonce.as_deref().filter(|_| mine).and_then(|n| log.pending(n));
        match pending {
            Some(ix) => {
                log.messages[ix] = message.into();
                if let Some(v) = view {
                    v.refresh(ix);
                }
            }
            None => log.push(message.into(), view),
        }
    }

    /// MESSAGE_UPDATE: applies whatever it carries.
    pub fn edit(&mut self, update: MessageUpdate, view: Option<&dyn MessageView>) {
        let Some(log) = self.logs.get_mut(&update.channel_id) else { return };
        let Some(ix) = log.position(update.id) else { return };
        let msg = &mut log.messages[ix];
        if let Some(content) = update.content {
            msg.content = content.into();
        }
        msg.edited |= update.edited_timestamp.is_some();
        if let Some(pinned) = update.pinned {
            msg.pinned = pinned;
        }
        if let Some(embeds) = update.embeds {
            msg.embeds = embeds;
        }
        if let Some(mentions) = update.mentions {
            msg.mentions = mention_names(&mentions);
        }
        if let Some(roles) = update.mention_roles {
            msg.mention_roles = roles;
        }
        if let Some(everyone) = update.mention_everyone {
            msg.mention_everyone = everyone;
        }
        if let Some(thread) = update.thread {
            msg.thread = Some(*thread);
        }
        if let Some(poll) = update.poll {
            msg.poll = Some(poll);
        }
        if let Some(v) = view {
            v.refresh(ix);
        }
    }

    /// The message `id` in `channel`, with its position in the log.
    pub fn find(&self, channel: Snowflake, id: Snowflake) -> Option<(usize, &Msg)> {
        let log = self.logs.get(&channel)?;
        let ix = log.position(id)?;
        Some((ix, &log.messages[ix]))
    }

    /// The newest message by `author` that can be edited here.
    pub fn last_editable(&self, channel: Snowflake, author: Snowflake) -> Option<Snowflake> {
        let log = self.logs.get(&channel)?;
        log.messages
            .iter()
            .rev()
            .find(|m| m.author_id == author && m.editable_text().is_some())
            .map(|m| m.id)
    }

    /// Applies `change` to the message `id`, redrawing it when `change`
    /// reports a difference.
    pub fn update(
        &mut self,
        channel: Snowflake,
        id: Snowflake,
        view: Option<&dyn MessageView>,
        change: impl FnOnce(&mut Msg) -> bool,
    ) {
        let Some(log) = self.logs.get_mut(&channel) else { return };
        let Some(ix) = log.position(id) else { return };
        if change(&mut log.messages[ix])
            && let Some(view) = view
        {
            view.refresh(ix);
        }
    }

    /// Redraws the message `id` if it is in the log `view` shows.
    pub fn redraw(&self, channel: Snowflake, id: Snowflake, view: Option<&dyn MessageView>) {
        if let (Some((ix, _)), Some(view)) = (self.find(channel, id), view) {
            view.refresh(ix);
        }
    }

    pub fn delete(&mut self, delete: MessageDelete, view: Option<&dyn MessageView>) {
        let Some(log) = self.logs.get_mut(&delete.channel_id) else { return };
        if let Some(ix) = log.position(delete.id) {
            log.messages.remove(ix);
            if let Some(v) = view {
                v.remove(ix);
            }
        }
    }

    pub fn add_pending(&mut self, channel: Snowflake, msg: Msg, view: Option<&dyn MessageView>) {
        if let Some(log) = self.logs.get_mut(&channel) {
            log.push(msg, view);
        }
    }

    /// Removes the pending copy of a send that failed.
    pub fn drop_pending(&mut self, channel: Snowflake, nonce: &str, view: Option<&dyn MessageView>) {
        let Some(log) = self.logs.get_mut(&channel) else { return };
        if let Some(ix) = log.pending(nonce) {
            log.messages.remove(ix);
            if let Some(v) = view {
                v.remove(ix);
            }
        }
    }

    pub fn forget(&mut self, channel: Snowflake, view: Option<&dyn MessageView>) {
        self.logs.remove(&channel);
        self.recent.retain(|c| *c != channel);
        if let Some(v) = view {
            v.reset(0);
        }
    }

    /// Newest confirmed message in the log.
    pub fn newest(&self, channel: Snowflake) -> Option<Snowflake> {
        let log = self.logs.get(&channel)?;
        log.messages.iter().rev().find(|m| !m.pending).map(|m| m.id)
    }

    pub fn authors(&self, channel: Snowflake) -> impl Iterator<Item = Snowflake> + '_ {
        self.logs
            .get(&channel)
            .into_iter()
            .flat_map(|log| log.messages.iter().map(|m| m.author_id))
    }
}

#[cfg(test)]
mod tests;
