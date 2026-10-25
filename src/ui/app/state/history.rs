//! Message history of the channels viewed recently, and the contract that
//! keeps the on-screen list in step with it.

use std::collections::{HashMap, HashSet, VecDeque};

use super::msg::{Msg, mention_names};
use crate::discord::Page;
use crate::discord::model::{Message, MessageDelete, MessageUpdate, Snowflake};
use crate::discord::rest::HISTORY_PAGE;

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

    /// A page of history: the newest one, or one around a message, replaces
    /// the log; older ones go on top and newer ones below, up to the cap.
    pub fn page(&mut self, channel: Snowflake, page: Page, messages: Vec<Message>, view: Option<&dyn MessageView>) {
        let log = self.logs.entry(channel).or_default();
        log.loading = false;
        let short = messages.len() < HISTORY_PAGE;
        match page {
            Page::Latest | Page::Around(_) => {
                // Around a message, a short page is a channel smaller than a
                // page: both ends are in.
                log.complete = short;
                log.latest = short || page == Page::Latest;
                log.messages = messages.into_iter().map(Msg::from).collect();
                if let Some(v) = view {
                    v.reset(log.messages.len());
                }
                return;
            }
            Page::After(_) => {
                log.latest = short;
                let known: HashSet<Snowflake> = log.messages.iter().map(|m| m.id).collect();
                let fresh: Vec<Msg> = messages
                    .into_iter()
                    .filter(|m| !known.contains(&m.id))
                    .map(Msg::from)
                    .collect();
                let end = log.messages.len();
                let added = fresh.len();
                log.messages.extend(fresh);
                if let Some(v) = view
                    && added > 0
                {
                    v.insert(end, added);
                }
                log.trim_front(view);
                return;
            }
            Page::Before(_) => log.complete = short,
        }
        let room = MESSAGES_PER_CHANNEL.saturating_sub(log.messages.len());
        let skip = messages.len().saturating_sub(room);
        let added = messages.len() - skip;
        if added == 0 {
            log.complete = true;
        }
        for message in messages.into_iter().skip(skip).rev() {
            log.messages.push_front(message.into());
        }
        if let Some(view) = view {
            if added > 0 {
                view.prepend(added);
            }
            if log.complete {
                view.refresh_intro();
            }
        }
    }

    /// A page request failed; `forbidden` stops further requests.
    pub fn page_failed(&mut self, channel: Snowflake, forbidden: bool, view: Option<&dyn MessageView>) {
        if let Some(log) = self.logs.get_mut(&channel) {
            log.loading = false;
        }
        if forbidden {
            self.forbidden.insert(channel);
            self.forget(channel, view);
        }
    }

    /// The newest message to page forward from in a log away from the
    /// present, marking it as loading; `None` while loading or at the
    /// present.
    pub fn newer(&mut self, channel: Snowflake) -> Option<Snowflake> {
        let log = self.logs.get_mut(&channel)?;
        if log.loading || log.latest {
            return None;
        }
        let newest = log.messages.back()?.id;
        log.loading = true;
        Some(newest)
    }

    /// Whether the log of `channel` reaches its newest message.
    pub fn at_latest(&self, channel: Snowflake) -> bool {
        self.logs.get(&channel).is_none_or(|log| log.latest)
    }

    /// Marks `channel`'s log as loading a page asked for elsewhere, as a
    /// jump does; false when one is already on its way.
    pub fn start_loading(&mut self, channel: Snowflake) -> bool {
        match self.logs.get_mut(&channel) {
            Some(log) if !log.loading => {
                log.loading = true;
                true
            }
            _ => false,
        }
    }

    /// The oldest message to page back from, marking the log as loading;
    /// `None` while loading, at the start, or at the cap.
    pub fn older(&mut self, channel: Snowflake) -> Option<Snowflake> {
        let log = self.logs.get_mut(&channel)?;
        if log.loading || log.complete || log.messages.len() >= MESSAGES_PER_CHANNEL {
            return None;
        }
        let oldest = log.messages.front()?.id;
        log.loading = true;
        Some(oldest)
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
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::discord::model::User;

    /// Mirrors the view's row count, as the list would.
    #[derive(Default)]
    struct Rows {
        count: RefCell<usize>,
        intro: RefCell<usize>,
        refreshed: RefCell<Vec<usize>>,
    }

    impl MessageView for Rows {
        fn reset(&self, messages: usize) {
            *self.count.borrow_mut() = messages;
        }
        fn insert(&self, _: usize, count: usize) {
            *self.count.borrow_mut() += count;
        }
        fn remove(&self, _: usize) {
            *self.count.borrow_mut() -= 1;
        }
        fn refresh(&self, at: usize) {
            self.refreshed.borrow_mut().push(at);
        }
        fn prepend(&self, count: usize) {
            *self.count.borrow_mut() += count;
        }
        fn refresh_intro(&self) {
            *self.intro.borrow_mut() += 1;
        }
    }

    const CHANNEL: Snowflake = Snowflake(9);

    fn me() -> User {
        User {
            id: Snowflake(1),
            username: "eu".into(),
            global_name: None,
            avatar: None,
            email: None,
            bot: false,
            premium_type: None,
        }
    }

    fn message(id: u64, nonce: Option<&str>) -> Message {
        serde_json::from_value(serde_json::json!({
            "id": id.to_string(),
            "channel_id": CHANNEL.to_string(),
            "author": { "id": "1", "username": "eu" },
            "content": format!("m{id}"),
            "nonce": nonce,
        }))
        .unwrap()
    }

    fn page(ids: std::ops::Range<u64>) -> Vec<Message> {
        ids.map(|id| message(id, None)).collect()
    }

    #[test]
    fn opening_fetches_once_and_only_with_permission() {
        let mut history = History::default();
        let rows = Rows::default();
        assert!(history.open(CHANNEL, true, &rows));
        assert!(!history.open(CHANNEL, true, &rows));
        assert!(!history.open(Snowflake(10), false, &rows));
        assert!(history.get(Snowflake(10)).unwrap().complete);
    }

    #[test]
    fn pending_send_is_confirmed_in_place() {
        let mut history = History::default();
        let rows = Rows::default();
        history.open(CHANNEL, false, &rows);
        history.add_pending(CHANNEL, Msg::pending(&me(), "oi".into(), "n1".into(), None), Some(&rows));
        history.receive(message(5, Some("n1")), true, Some(&rows));
        // The echo arriving twice (gateway and REST) is dropped.
        history.receive(message(5, Some("n1")), true, Some(&rows));
        let log = history.get(CHANNEL).unwrap();
        assert_eq!((log.messages.len(), *rows.count.borrow()), (1, 1));
        assert!(!log.messages[0].pending);
        assert_eq!(*rows.refreshed.borrow(), [0]);
    }

    #[test]
    fn updates_apply_only_what_they_carry() {
        let mut history = History::default();
        let rows = Rows::default();
        history.open(CHANNEL, false, &rows);
        history.receive(message(5, None), false, Some(&rows));
        let update = |json: serde_json::Value| serde_json::from_value::<MessageUpdate>(json).unwrap();
        history.edit(update(serde_json::json!({ "id": "5", "channel_id": "9", "pinned": true })), Some(&rows));
        let msg = history.find(CHANNEL, Snowflake(5)).unwrap().1;
        assert!(msg.pinned && !msg.edited);
        assert_eq!(msg.content.as_ref(), "m5");
        history.edit(
            update(serde_json::json!({
                "id": "5", "channel_id": "9", "content": "novo", "edited_timestamp": "2026-10-20T10:00:00Z"
            })),
            Some(&rows),
        );
        let msg = history.find(CHANNEL, Snowflake(5)).unwrap().1;
        assert!(msg.pinned && msg.edited);
        assert_eq!(msg.content.as_ref(), "novo");
        assert_eq!(history.last_editable(CHANNEL, Snowflake(1)), Some(Snowflake(5)));
        assert_eq!(history.last_editable(CHANNEL, Snowflake(2)), None);
    }

    #[test]
    fn failed_send_removes_its_copy() {
        let mut history = History::default();
        let rows = Rows::default();
        history.open(CHANNEL, false, &rows);
        history.add_pending(CHANNEL, Msg::pending(&me(), "oi".into(), "n1".into(), None), Some(&rows));
        history.drop_pending(CHANNEL, "n1", Some(&rows));
        assert_eq!(*rows.count.borrow(), 0);
    }

    #[test]
    fn older_pages_stop_at_the_start() {
        let mut history = History::default();
        let rows = Rows::default();
        history.open(CHANNEL, true, &rows);
        history.page(CHANNEL, Page::Latest, page(100..150), Some(&rows));
        assert_eq!(history.older(CHANNEL), Some(Snowflake(100)));
        // Already loading: no second request.
        assert_eq!(history.older(CHANNEL), None);
        history.page(CHANNEL, Page::Before(Snowflake(100)), page(80..100), Some(&rows));
        let log = history.get(CHANNEL).unwrap();
        assert!(log.complete);
        assert_eq!(log.messages.front().map(|m| m.id), Some(Snowflake(80)));
        assert_eq!((*rows.count.borrow(), *rows.intro.borrow()), (70, 1));
        assert_eq!(history.older(CHANNEL), None);
    }

    #[test]
    fn jumps_detach_from_the_present_until_paged_back() {
        let mut history = History::default();
        let rows = Rows::default();
        history.open(CHANNEL, true, &rows);
        history.page(CHANNEL, Page::Around(Snowflake(500)), page(475..525), Some(&rows));
        assert!(!history.at_latest(CHANNEL));
        // New messages wait for paging forward to bring them in order.
        history.receive(message(900, None), false, Some(&rows));
        assert_eq!(history.get(CHANNEL).unwrap().messages.len(), 50);
        assert_eq!(history.newer(CHANNEL), Some(Snowflake(524)));
        assert_eq!(history.newer(CHANNEL), None);
        history.page(CHANNEL, Page::After(Snowflake(524)), page(524..540), Some(&rows));
        let log = history.get(CHANNEL).unwrap();
        assert!(log.latest);
        assert_eq!((log.messages.len(), *rows.count.borrow()), (65, 65));
        history.receive(message(900, None), false, Some(&rows));
        assert_eq!(history.get(CHANNEL).unwrap().messages.back().map(|m| m.id), Some(Snowflake(900)));
    }

    #[test]
    fn small_channels_are_whole_around_any_message() {
        let mut history = History::default();
        let rows = Rows::default();
        history.open(CHANNEL, true, &rows);
        history.page(CHANNEL, Page::Around(Snowflake(5)), page(1..10), Some(&rows));
        let log = history.get(CHANNEL).unwrap();
        assert!(log.latest && log.complete);
    }

    #[test]
    fn logs_are_capped() {
        let mut history = History::default();
        let rows = Rows::default();
        history.open(CHANNEL, false, &rows);
        for id in 0..MESSAGES_PER_CHANNEL as u64 + 5 {
            history.receive(message(id + 1, None), false, Some(&rows));
        }
        let log = history.get(CHANNEL).unwrap();
        assert_eq!(log.messages.len(), MESSAGES_PER_CHANNEL);
        assert_eq!(*rows.count.borrow(), MESSAGES_PER_CHANNEL);
        assert!(!log.complete);
        assert_eq!(history.newest(CHANNEL), Some(Snowflake(MESSAGES_PER_CHANNEL as u64 + 5)));
    }

    #[test]
    fn least_recent_logs_are_evicted() {
        let mut history = History::default();
        let rows = Rows::default();
        for id in 0..CACHED_CHANNELS as u64 + 1 {
            history.open(Snowflake(id), false, &rows);
        }
        assert!(history.get(Snowflake(0)).is_none());
        assert!(history.get(Snowflake(1)).is_some());
    }
}
