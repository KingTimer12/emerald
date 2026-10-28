//! Moderation and channel settings: the bodies of the calls a moderator
//! makes. Field names follow the web client's requests.

use serde_json::{Map, Value, json};

use crate::model::Snowflake;
use crate::protocol::{ChannelEdit, TagEdit};

/// `POST /channels/{id}/messages/bulk-delete`. Discord takes 2 to 100
/// messages no older than two weeks.
pub fn bulk_delete(messages: &[Snowflake]) -> Value {
    json!({ "messages": messages.iter().map(Snowflake::to_string).collect::<Vec<_>>() })
}

/// `PUT /guilds/{id}/bans/{user}`: `delete_seconds` of their messages go too.
pub fn ban(delete_seconds: u32) -> Value {
    json!({ "delete_message_seconds": delete_seconds })
}

/// `PATCH /guilds/{id}/members/{user}`: a timeout until `until`, or none.
pub fn timeout(until: Option<&str>) -> Value {
    json!({ "communication_disabled_until": until })
}

/// `PATCH /channels/{id}` with only the fields that are set.
pub fn channel(edit: &ChannelEdit) -> Value {
    let mut body = Map::new();
    if let Some(name) = &edit.name {
        body.insert("name".into(), json!(name));
    }
    if let Some(topic) = &edit.topic {
        body.insert("topic".into(), json!(topic));
    }
    if let Some(slow_mode) = edit.slow_mode {
        body.insert("rate_limit_per_user".into(), json!(slow_mode));
    }
    if let Some(tags) = &edit.tags {
        body.insert("available_tags".into(), Value::Array(tags.iter().map(tag).collect()));
    }
    if let Some(archived) = edit.archived {
        body.insert("archived".into(), json!(archived));
    }
    if let Some(locked) = edit.locked {
        body.insert("locked".into(), json!(locked));
    }
    Value::Object(body)
}

fn tag(tag: &TagEdit) -> Value {
    let mut body = Map::new();
    if let Some(id) = tag.id {
        body.insert("id".into(), json!(id.to_string()));
    }
    body.insert("name".into(), json!(tag.name));
    body.insert("moderated".into(), json!(tag.moderated));
    if let Some(emoji_id) = tag.emoji_id {
        body.insert("emoji_id".into(), json!(emoji_id.to_string()));
    }
    if let Some(emoji_name) = &tag.emoji_name {
        body.insert("emoji_name".into(), json!(emoji_name));
    }
    Value::Object(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bulk_delete_lists_ids_as_strings() {
        let body = bulk_delete(&[Snowflake(1), Snowflake(22)]);
        assert_eq!(body, json!({ "messages": ["1", "22"] }));
    }

    #[test]
    fn timeout_none_lifts_it() {
        assert_eq!(timeout(None), json!({ "communication_disabled_until": null }));
        assert_eq!(timeout(Some("2030-01-01T00:00:00Z"))["communication_disabled_until"], "2030-01-01T00:00:00Z");
    }

    #[test]
    fn channel_sends_only_changed_fields() {
        let edit = ChannelEdit { slow_mode: Some(10), ..Default::default() };
        assert_eq!(channel(&edit), json!({ "rate_limit_per_user": 10 }));
        let thread = ChannelEdit { archived: Some(true), locked: Some(false), ..Default::default() };
        assert_eq!(channel(&thread), json!({ "archived": true, "locked": false }));
    }

    #[test]
    fn new_tags_have_no_id() {
        let edit = ChannelEdit {
            tags: Some(vec![
                TagEdit { id: Some(Snowflake(7)), name: "bug".into(), moderated: true, emoji_id: None, emoji_name: Some("🐛".into()) },
                TagEdit { id: None, name: "new".into(), moderated: false, emoji_id: None, emoji_name: None },
            ]),
            ..Default::default()
        };
        assert_eq!(
            channel(&edit)["available_tags"],
            json!([
                { "id": "7", "name": "bug", "moderated": true, "emoji_name": "🐛" },
                { "name": "new", "moderated": false },
            ])
        );
    }
}
