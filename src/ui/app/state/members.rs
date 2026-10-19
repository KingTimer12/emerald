//! The member sidebar of one channel, loaded lazily. Guilds with millions
//! of members are never downloaded: the server keeps the list and sends
//! operations for the 100-row pages the client subscribes to, so only the
//! rows near the viewport are held, the rest stay empty slots.

use std::ops::Range;

use crate::discord::model::{MemberListGroup, MemberListItem, MemberListOp, MemberListUpdate, Overwrite, Snowflake};
use crate::discord::permissions;

/// Rows per subscribed range, as the web client asks for.
const PAGE: usize = 100;

pub struct MemberList {
    pub guild: Snowflake,
    pub channel: Snowflake,
    /// The server's id for this list; channels with the same view
    /// overwrites share one.
    id: String,
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
    pub fn new(guild: Snowflake, channel: Snowflake, overwrites: &[Overwrite]) -> Self {
        Self {
            guild,
            channel,
            id: list_id(overwrites),
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
            log::debug!("member list id {} is {}", self.id, update.id);
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

/// The id Discord gives a channel's member list: `everyone` when no
/// overwrite touches VIEW_CHANNEL, else a MurmurHash3 of those overwrites.
fn list_id(overwrites: &[Overwrite]) -> String {
    let mut parts: Vec<String> = overwrites
        .iter()
        .filter_map(|o| {
            if o.allow & permissions::VIEW_CHANNEL != 0 {
                Some(format!("allow:{}", o.id))
            } else if o.deny & permissions::VIEW_CHANNEL != 0 {
                Some(format!("deny:{}", o.id))
            } else {
                None
            }
        })
        .collect();
    if parts.is_empty() {
        return "everyone".into();
    }
    parts.sort();
    murmur3(parts.join(",").as_bytes(), 0).to_string()
}

/// MurmurHash3, x86 32-bit variant.
fn murmur3(data: &[u8], seed: u32) -> u32 {
    const C1: u32 = 0xcc9e_2d51;
    const C2: u32 = 0x1b87_3593;
    let mix = |k: u32| k.wrapping_mul(C1).rotate_left(15).wrapping_mul(C2);
    let mut hash = seed;
    let (chunks, tail) = data.as_chunks::<4>();
    for chunk in chunks {
        hash ^= mix(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
        hash = hash.rotate_left(13).wrapping_mul(5).wrapping_add(0xe654_6b64);
    }
    if !tail.is_empty() {
        let k = tail.iter().rev().fold(0u32, |k, byte| (k << 8) | *byte as u32);
        hash ^= mix(k);
    }
    hash ^= data.len() as u32;
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x85eb_ca6b);
    hash ^= hash >> 13;
    hash = hash.wrapping_mul(0xc2b2_ae35);
    hash ^ (hash >> 16)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discord::model::overwrite_kind;

    const GUILD: Snowflake = Snowflake(1);

    fn update(id: &str, groups: &[(&str, u64)], ops: serde_json::Value) -> MemberListUpdate {
        let groups: Vec<_> = groups
            .iter()
            .map(|(id, count)| serde_json::json!({ "id": id, "count": count }))
            .collect();
        serde_json::from_value(serde_json::json!({
            "id": id, "guild_id": "1", "groups": groups, "ops": ops,
        }))
        .unwrap()
    }

    fn member(id: u64) -> serde_json::Value {
        serde_json::json!({ "member": { "user": { "id": id.to_string(), "username": format!("u{id}") } } })
    }

    fn ids(list: &MemberList) -> Vec<Option<String>> {
        (0..list.len())
            .map(|ix| match list.row(ix) {
                Some(MemberListItem::Member(m)) => Some(m.user.username.clone()),
                Some(MemberListItem::Group(g)) => Some(g.id.clone()),
                None => None,
            })
            .collect()
    }

    #[test]
    fn murmur3_matches_reference() {
        assert_eq!(murmur3(b"", 0), 0);
        assert_eq!(murmur3(b"hello", 0), 613_153_351);
        assert_eq!(murmur3(b"The quick brown fox jumps over the lazy dog", 0), 0x2e4f_f723);
    }

    #[test]
    fn list_ids_depend_on_view_overwrites() {
        assert_eq!(list_id(&[]), "everyone");
        let deny = Overwrite {
            id: Snowflake(5),
            kind: overwrite_kind::ROLE,
            allow: 0,
            deny: permissions::VIEW_CHANNEL,
        };
        let unrelated = Overwrite {
            deny: permissions::SEND_MESSAGES,
            ..deny.clone()
        };
        assert_eq!(list_id(&[unrelated]), "everyone");
        assert_eq!(list_id(&[deny]), murmur3(b"deny:5", 0).to_string());
    }

    #[test]
    fn sync_then_edits_keep_a_sparse_list() {
        let mut list = MemberList::new(GUILD, Snowflake(2), &[]);
        let sync = serde_json::json!([{ "op": "SYNC", "range": [0, 99],
            "items": [{ "group": { "id": "online", "count": 2 } }, member(10), member(11)] }]);
        assert!(list.apply(update("everyone", &[("online", 2), ("offline", 1000)], sync)));
        // Header plus members of both groups, most of them not loaded.
        assert_eq!(list.len(), 3 + 1001);
        assert_eq!(ids(&list)[..4], [Some("online".into()), Some("u10".into()), Some("u11".into()), None]);

        let edits = serde_json::json!([
            { "op": "DELETE", "index": 1 },
            { "op": "INSERT", "index": 2, "item": member(12) },
            { "op": "UPDATE", "index": 1, "item": member(13) },
        ]);
        assert!(list.apply(update("everyone", &[("online", 2), ("offline", 1000)], edits)));
        assert_eq!(ids(&list)[..3], [Some("online".into()), Some("u13".into()), Some("u12".into())]);
        assert_eq!(list.len(), 1004);
    }

    #[test]
    fn other_lists_are_ignored_once_synced() {
        let mut list = MemberList::new(GUILD, Snowflake(2), &[]);
        let sync = || serde_json::json!([{ "op": "SYNC", "range": [0, 99], "items": [member(1)] }]);
        // A mismatched id is adopted on the first sync from the top...
        assert!(list.apply(update("123", &[("online", 0)], sync())));
        // ...but not afterwards.
        assert!(!list.apply(update("456", &[("online", 0)], sync())));
    }

    #[test]
    fn scrolling_moves_ranges_and_frees_rows() {
        let mut list = MemberList::new(GUILD, Snowflake(2), &[]);
        assert_eq!(list.ranges(), [[0, 99]]);
        assert_eq!(list.scrolled(0..20), None);
        let sync = serde_json::json!([{ "op": "SYNC", "range": [0, 99], "items": [member(1)] }]);
        list.apply(update("everyone", &[("online", 500)], sync));
        assert_eq!(list.scrolled(190..215), Some(vec![[0, 99], [100, 199], [200, 299]]));
        assert_eq!(list.scrolled(450..470), Some(vec![[0, 99], [400, 499]]));
        // The first page stays; everything else outside the ranges is dropped.
        assert!(list.row(0).is_some());
    }
}
