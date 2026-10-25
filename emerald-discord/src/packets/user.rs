//! The account and its relationships: friend requests, blocks, nicknames,
//! notes, DMs and group DMs, the display name and logging out.

use serde_json::{Value, json};

use crate::model::Snowflake;

pub fn friend_request(username: &str) -> Value {
    json!({ "username": username, "discriminator": null })
}

/// Accepting an incoming friend request.
pub fn accept_friend() -> Value {
    json!({})
}

/// Opens a DM with one user, or starts a group DM with several.
pub fn open_dm(users: &[Snowflake]) -> Value {
    let recipients: Vec<String> = users.iter().map(Snowflake::to_string).collect();
    json!({ "recipients": recipients })
}

/// "Block": the relationship becomes type 2.
pub fn block() -> Value {
    json!({ "type": crate::model::relationship_kind::BLOCKED })
}

/// A friend's nickname; `None` clears it.
pub fn nickname(nickname: Option<&str>) -> Value {
    json!({ "nickname": nickname })
}

/// The note on a user's profile; empty clears it.
pub fn note(note: &str) -> Value {
    json!({ "note": note })
}

/// Accepting a message request: `consent_status` 2, ACCEPTED.
pub fn accept_request() -> Value {
    json!({ "consent_status": 2 })
}

/// A group DM's new name; `None` names it after its members again.
pub fn group_name(name: Option<&str>) -> Value {
    json!({ "name": name })
}

/// An image as the `data:` URI Discord takes for icons.
pub fn data_uri(mime: &str, bytes: &[u8]) -> String {
    use base64::Engine;
    format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// A group DM's new icon as a `data:` URI; `None` removes it.
pub fn group_icon(icon: Option<&str>) -> Value {
    json!({ "icon": icon })
}

/// `None` clears the display name back to the username.
pub fn global_name(name: Option<&str>) -> Value {
    json!({ "global_name": name })
}

/// Revokes the token, as the web client does on "Log Out".
pub fn logout() -> Value {
    json!({ "provider": null, "voip_provider": null })
}
