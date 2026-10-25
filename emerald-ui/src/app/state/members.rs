//! The member sidebar, loaded lazily. Guilds with millions of members are
//! never downloaded: the server keeps the list and sends operations for the
//! 100-row pages the client subscribes to, so only the rows near the
//! viewport are held, the rest stay empty slots.
//!
//! Channels with the same view permissions share one server-side list, and
//! the gateway syncs a list only the first time it is subscribed in a
//! session: it keeps sending updates for every list seen since, and switching
//! back to one brings no new SYNC. So, like the web client, lists are kept
//! per guild and list id for the whole session and reused when a channel
//! shows one already seen.

mod list_id;

pub use list_id::*;

use std::ops::Range;

use emerald_discord::model::{MemberListGroup, MemberListItem, MemberListOp, MemberListUpdate, Overwrite, Snowflake};
use emerald_discord::permissions;

/// Rows per subscribed range, as the web client asks for.
const PAGE: usize = 100;

pub struct MemberList {
    pub guild: Snowflake,
    pub channel: Snowflake,
    /// The server's id for this list; channels with the same view
    /// overwrites share one.
    id: String,
    /// The id computed from the overwrites, which finds the list again
    /// should the server's ever differ.
    computed: String,
    /// An update was applied, so `id` is confirmed.
    synced: bool,
    groups: Vec<MemberListGroup>,
    /// One slot per row of the whole list; `None` outside loaded pages.
    rows: Vec<Option<MemberListItem>>,
    ranges: Vec<[usize; 2]>,
    pub member_count: u64,
    pub online_count: u64,
}

impl MemberList {
    /// `id` comes from `list_id`.
    pub fn new(guild: Snowflake, channel: Snowflake, id: String) -> Self {
        Self {
            guild,
            channel,
            computed: id.clone(),
            id,
            synced: false,
            groups: Vec::new(),
            rows: Vec::new(),
            ranges: ranges_for(0..1),
            member_count: 0,
            online_count: 0,
        }
    }

    /// The row ranges to subscribe to.
    pub fn ranges(&self) -> &[[usize; 2]] {
        &self.ranges
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn row(&self, ix: usize) -> Option<&MemberListItem> {
        self.rows.get(ix)?.as_ref()
    }

    pub fn group(&self, id: &str) -> Option<&MemberListGroup> {
        self.groups.iter().find(|g| g.id == id)
    }

    /// Applies `update` when it is for this list; false otherwise.
    pub fn apply(&mut self, update: MemberListUpdate) -> bool {
        if update.guild_id != self.guild {
            return false;
        }
        if update.id != self.id {
            // Should the computed id ever differ from the server's, the
            // first page synced from the top after subscribing is ours.
            let from_top = update
                .ops
                .iter()
                .any(|op| matches!(op, MemberListOp::Sync { range: [0, _], .. }));
            if self.synced || !from_top {
                return false;
            }
            tracing::debug!("member list id {} is {}", self.id, update.id);
            self.id = update.id;
        }
        self.synced = true;
        for op in update.ops {
            self.apply_op(op);
        }
        self.groups = update.groups;
        self.member_count = update.member_count;
        self.online_count = update.online_count;
        let total = self.groups.iter().map(|g| g.count as usize + 1).sum();
        self.rows.resize(total, None);
        true
    }

    fn apply_op(&mut self, op: MemberListOp) {
        let slot = |rows: &mut Vec<Option<MemberListItem>>, ix: usize| {
            if rows.len() <= ix {
                rows.resize(ix + 1, None);
            }
        };
        match op {
            MemberListOp::Sync { range, items } => {
                slot(&mut self.rows, range[1].max(range[0] + items.len().saturating_sub(1)));
                for (ix, item) in items.into_iter().enumerate() {
                    self.rows[range[0] + ix] = Some(item);
                }
            }
            MemberListOp::Insert { index, item } => {
                slot(&mut self.rows, index.saturating_sub(1));
                self.rows.insert(index.min(self.rows.len()), Some(item));
            }
            MemberListOp::Update { index, item } => {
                slot(&mut self.rows, index);
                self.rows[index] = Some(item);
            }
            MemberListOp::Delete { index } => {
                if index < self.rows.len() {
                    self.rows.remove(index);
                }
            }
            MemberListOp::Invalidate { range } => {
                let end = (range[1] + 1).min(self.rows.len());
                for row in self.rows.get_mut(range[0].min(end)..end).into_iter().flatten() {
                    *row = None;
                }
            }
            MemberListOp::Unknown => {}
        }
    }

    /// The rows on screen moved. When they reach other pages, returns the
    /// new ranges to subscribe to and forgets rows outside them, which keeps
    /// memory bounded however far the user scrolls.
    pub fn scrolled(&mut self, visible: Range<usize>) -> Option<Vec<[usize; 2]>> {
        let ranges = ranges_for(visible);
        if ranges == self.ranges {
            return None;
        }
        let kept = |ix: usize| ranges.iter().any(|[start, end]| (*start..=*end).contains(&ix));
        for (ix, row) in self.rows.iter_mut().enumerate() {
            if !kept(ix) {
                *row = None;
            }
        }
        self.ranges = ranges.clone();
        Some(ranges)
    }
}

/// Every member list seen this session; at most one is on screen.
#[derive(Default)]
pub struct MemberLists {
    lists: Vec<MemberList>,
    current: Option<usize>,
}

impl MemberLists {
    pub fn current(&self) -> Option<&MemberList> {
        self.lists.get(self.current?)
    }

    pub fn current_mut(&mut self) -> Option<&mut MemberList> {
        self.lists.get_mut(self.current?)
    }

    /// Shows the list of `channel`, whose id `list_id` gives, reusing the
    /// one already held when another channel shares it. False when nothing
    /// changed.
    pub fn open(&mut self, guild: Snowflake, channel: Snowflake, id: String) -> bool {
        if self.current().is_some_and(|list| list.channel == channel) {
            return false;
        }
        let held = self
            .lists
            .iter()
            .position(|list| list.guild == guild && (list.computed == id || list.id == id));
        let ix = held.unwrap_or_else(|| {
            self.lists.push(MemberList::new(guild, channel, id));
            self.lists.len() - 1
        });
        self.lists[ix].channel = channel;
        self.current = Some(ix);
        true
    }

    /// Hides the list; the lists stay, the server still updates them.
    pub fn close(&mut self) {
        self.current = None;
    }

    /// A new session subscribes from scratch, so nothing held is current.
    pub fn clear(&mut self) {
        self.lists.clear();
        self.current = None;
    }

    /// Applies `update` to the list it is for.
    pub fn apply(&mut self, update: MemberListUpdate) -> bool {
        let exact = self
            .lists
            .iter()
            .position(|list| list.guild == update.guild_id && list.id == update.id);
        match exact.or(self.current) {
            Some(ix) => self.lists[ix].apply(update),
            None => false,
        }
    }
}

/// The first page, always kept like the web client does, plus the pages
/// the visible rows fall in.
fn ranges_for(visible: Range<usize>) -> Vec<[usize; 2]> {
    let first = visible.start / PAGE;
    let last = (visible.end.saturating_sub(1) / PAGE).max(first);
    std::iter::once(0)
        .chain((first..=last.min(first + 1)).filter(|page| *page != 0))
        .map(|page| [page * PAGE, page * PAGE + PAGE - 1])
        .collect()
}

#[cfg(test)]
mod tests;
