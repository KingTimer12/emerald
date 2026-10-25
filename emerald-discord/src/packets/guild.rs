//! Servers: joining through an invite and creating one.

use serde_json::{Value, json};

use crate::model::Invite;

/// Template the web client uses for "Create My Own".
const CREATE_GUILD_TEMPLATE: &str = "2TffvPucqHkN";

/// `POST /invites/{code}`.
pub fn join(session: Option<String>) -> Value {
    json!({ "session_id": session })
}

/// The `X-Context-Properties` of a join: where the invite leads.
pub fn join_context(invite: &Invite) -> Value {
    json!({
        "location": "Join Guild",
        "location_guild_id": invite.guild.as_ref().map(|g| g.id.to_string()),
        "location_channel_id": invite.channel.as_ref().map(|c| c.id.to_string()),
        "location_channel_type": invite.channel.as_ref().map(|c| c.kind),
    })
}

/// "Create My Own", with no icon.
pub fn create(name: &str) -> Value {
    json!({
        "name": name,
        "icon": null,
        "channels": [],
        "system_channel_id": null,
        "guild_template_code": CREATE_GUILD_TEMPLATE,
    })
}
