use super::*;
use emerald_discord::model::overwrite_kind;

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
    const VIEW: u64 = permissions::VIEW_CHANNEL;
    assert_eq!(list_id(VIEW, &[]), "everyone");
    let deny = Overwrite {
        id: Snowflake(5),
        kind: overwrite_kind::ROLE,
        allow: 0,
        deny: VIEW,
    };
    let unrelated = Overwrite {
        deny: permissions::SEND_MESSAGES,
        ..deny.clone()
    };
    assert_eq!(list_id(VIEW, &[unrelated]), "everyone");
    // Any overwrite that hides the channel names the list...
    assert_eq!(list_id(VIEW, std::slice::from_ref(&deny)), murmur3(b"deny:5", 0).to_string());
    // ...as does an @everyone role that cannot see channels at all,
    // even where an overwrite lets it see this channel.
    let allow = Overwrite {
        id: GUILD,
        kind: overwrite_kind::ROLE,
        allow: VIEW,
        deny: 0,
    };
    assert_eq!(list_id(0, std::slice::from_ref(&allow)), murmur3(b"allow:1", 0).to_string());
    assert_eq!(list_id(VIEW, &[allow]), "everyone");
}

#[test]
fn sync_then_edits_keep_a_sparse_list() {
    let mut list = MemberList::new(GUILD, Snowflake(2), "everyone".into());
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
    let mut list = MemberList::new(GUILD, Snowflake(2), "everyone".into());
    let sync = || serde_json::json!([{ "op": "SYNC", "range": [0, 99], "items": [member(1)] }]);
    // A mismatched id is adopted on the first sync from the top...
    assert!(list.apply(update("123", &[("online", 0)], sync())));
    // ...but not afterwards.
    assert!(!list.apply(update("456", &[("online", 0)], sync())));
}

#[test]
fn channels_sharing_a_list_reuse_it() {
    let mut lists = MemberLists::default();
    assert!(lists.open(GUILD, Snowflake(2), "everyone".into()));
    let sync = serde_json::json!([{ "op": "SYNC", "range": [0, 99], "items": [member(1)] }]);
    assert!(lists.apply(update("everyone", &[("online", 1)], sync)));
    // Another channel with the same view: the server sends no new SYNC,
    // so the rows already held must show.
    assert!(lists.open(GUILD, Snowflake(3), "everyone".into()));
    assert_eq!(lists.current().unwrap().channel, Snowflake(3));
    assert!(lists.current().unwrap().row(0).is_some());
    assert!(!lists.open(GUILD, Snowflake(3), "everyone".into()));
}

#[test]
fn hidden_lists_keep_following_updates() {
    let mut lists = MemberLists::default();
    lists.open(GUILD, Snowflake(2), "everyone".into());
    let sync = || serde_json::json!([{ "op": "SYNC", "range": [0, 99], "items": [member(1)] }]);
    lists.apply(update("everyone", &[("online", 1)], sync()));
    let private = [Overwrite {
        id: Snowflake(5),
        kind: overwrite_kind::ROLE,
        allow: 0,
        deny: permissions::VIEW_CHANNEL,
    }];
    lists.open(GUILD, Snowflake(4), list_id(0, &private));
    assert!(lists.current().unwrap().row(0).is_none());
    // An update for the hidden list lands there, not on the one shown.
    let edit = serde_json::json!([{ "op": "UPDATE", "index": 0, "item": member(9) }]);
    assert!(lists.apply(update("everyone", &[("online", 1)], edit)));
    assert!(lists.current().unwrap().row(0).is_none());
    lists.open(GUILD, Snowflake(2), "everyone".into());
    assert!(lists.current().unwrap().row(0).is_some());
}

#[test]
fn scrolling_moves_ranges_and_frees_rows() {
    let mut list = MemberList::new(GUILD, Snowflake(2), "everyone".into());
    assert_eq!(list.ranges(), [[0, 99]]);
    assert_eq!(list.scrolled(0..20), None);
    let sync = serde_json::json!([{ "op": "SYNC", "range": [0, 99], "items": [member(1)] }]);
    list.apply(update("everyone", &[("online", 500)], sync));
    assert_eq!(list.scrolled(190..215), Some(vec![[0, 99], [100, 199], [200, 299]]));
    assert_eq!(list.scrolled(450..470), Some(vec![[0, 99], [400, 499]]));
    // The first page stays; everything else outside the ranges is dropped.
    assert!(list.row(0).is_some());
}
