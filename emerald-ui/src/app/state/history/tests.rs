use std::cell::RefCell;

use super::*;
use emerald_discord::model::User;

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
