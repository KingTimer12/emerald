//! Gateway frames: the envelope every frame comes in, and the ones sent,
//! each in the field order the web client writes.

use serde::Deserialize;
use serde_json::value::RawValue;
use serde_json::{Value, json};

use crate::identity::CAPABILITIES;
use crate::model::Snowflake;

/// Gateway opcodes this client sends or handles.
pub mod op {
    pub const DISPATCH: u8 = 0;
    pub const HEARTBEAT: u8 = 1;
    pub const IDENTIFY: u8 = 2;
    pub const PRESENCE_UPDATE: u8 = 3;
    pub const RESUME: u8 = 6;
    pub const RECONNECT: u8 = 7;
    pub const REQUEST_MEMBERS: u8 = 8;
    pub const INVALID_SESSION: u8 = 9;
    pub const HELLO: u8 = 10;
    pub const HEARTBEAT_ACK: u8 = 11;
    pub const GUILD_SUBSCRIPTIONS: u8 = 37;
}

/// Any frame: `d` stays raw until `op` and `t` say what it holds.
#[derive(Deserialize)]
pub struct Payload<'a> {
    pub op: u8,
    #[serde(borrow, default)]
    pub d: Option<&'a RawValue>,
    #[serde(default)]
    pub s: Option<u64>,
    #[serde(borrow, default)]
    pub t: Option<&'a str>,
}

/// Op 10, the first frame of a connection.
#[derive(Deserialize)]
pub struct Hello {
    pub heartbeat_interval: u64,
}

/// Op 2. Field order follows the web client's IDENTIFY; `qos_token` is
/// omitted.
pub fn identify(token: &str, properties: &Value, status: &str) -> Value {
    json!({
        "op": op::IDENTIFY,
        "d": {
            "token": token,
            "capabilities": CAPABILITIES,
            "properties": properties,
            "presence": { "status": status, "since": 0, "activities": [], "afk": false },
            "compress": false,
            "client_state": { "guild_versions": {} },
        },
    })
}

/// Op 6: picks `session` up after the last event seen, `seq`.
pub fn resume(token: &str, session: &str, seq: Option<u64>) -> Value {
    json!({
        "op": op::RESUME,
        "d": { "token": token, "session_id": session, "seq": seq },
    })
}

/// Op 1, with the last sequence number seen.
pub fn heartbeat(seq: Option<u64>) -> Value {
    json!({ "op": op::HEARTBEAT, "d": seq })
}

/// Op 3: our status, with no activities.
pub fn presence_update(status: &str) -> Value {
    json!({ "op": op::PRESENCE_UPDATE, "d": { "status": status, "since": 0, "activities": [], "afk": false } })
}

/// Op 8 for specific users, the shape the web client uses for message authors.
pub fn request_members(guild: Snowflake, users: &[Snowflake]) -> Value {
    let users: Vec<String> = users.iter().map(Snowflake::to_string).collect();
    json!({
        "op": op::REQUEST_MEMBERS,
        "d": { "guild_id": [guild.to_string()], "user_ids": users, "presences": false },
    })
}

/// Op 8 with a name prefix, as the member search box sends it.
pub fn search_members(guild: Snowflake, query: &str, nonce: &str) -> Value {
    json!({
        "op": op::REQUEST_MEMBERS,
        "d": { "guild_id": [guild.to_string()], "query": query, "limit": 100, "presences": false, "nonce": nonce },
    })
}

/// Op 8 as the message box's `@` suggestions send it: the first ten
/// members whose name starts with `query`, with no nonce.
pub fn query_members(guild: Snowflake, query: &str) -> Value {
    json!({
        "op": op::REQUEST_MEMBERS,
        "d": { "guild_id": [guild.to_string()], "query": query, "limit": 10, "presences": false },
    })
}

/// Op 37, as the web client sends on opening a guild: typing, activity and
/// thread events, plus the member list rows of `channel` in `ranges`.
pub fn subscribe_guild(guild: Snowflake, channel: Option<(Snowflake, &[[usize; 2]])>) -> Value {
    let mut subscription = json!({ "typing": true, "activities": true, "threads": true });
    if let Some((channel, ranges)) = channel {
        subscription["channels"] = json!({ channel.to_string(): ranges });
    }
    json!({
        "op": op::GUILD_SUBSCRIPTIONS,
        "d": { "subscriptions": { guild.to_string(): subscription } },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identify_keeps_the_web_client_order() {
        let sent = identify("t", &json!({ "os": "Mac OS X" }), "unknown").to_string();
        assert_eq!(
            sent,
            r#"{"op":2,"d":{"token":"t","capabilities":1734653,"properties":{"os":"Mac OS X"},"presence":{"status":"unknown","since":0,"activities":[],"afk":false},"compress":false,"client_state":{"guild_versions":{}}}}"#
        );
    }

    #[test]
    fn heartbeats_carry_the_sequence() {
        assert_eq!(heartbeat(Some(42)).to_string(), r#"{"op":1,"d":42}"#);
        assert_eq!(heartbeat(None).to_string(), r#"{"op":1,"d":null}"#);
    }
}
