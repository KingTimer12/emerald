//! The account and its relationships: friend requests, DMs, the display
//! name and logging out.

use serde_json::{Value, json};

use crate::model::Snowflake;

pub fn friend_request(username: &str) -> Value {
    json!({ "username": username, "discriminator": null })
}

/// Accepting an incoming friend request.
pub fn accept_friend() -> Value {
    json!({})
}

pub fn open_dm(user: Snowflake) -> Value {
    json!({ "recipients": [user.to_string()] })
}

/// `None` clears the display name back to the username.
pub fn global_name(name: Option<&str>) -> Value {
    json!({ "global_name": name })
}

/// Revokes the token, as the web client does on "Log Out".
pub fn logout() -> Value {
    json!({ "provider": null, "voip_provider": null })
}
