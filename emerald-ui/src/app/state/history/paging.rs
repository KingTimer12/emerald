//! Pages of history: older, newer and around a message, and when to ask for them.

use super::*;

impl History {
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
}
