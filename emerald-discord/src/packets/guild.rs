//! Servers: joining through an invite, creating one, inviting people and
//! leaving.

use serde_json::{Value, json};

use crate::model::{Invite, Snowflake};

/// Template the web client uses for "Create My Own".
const CREATE_GUILD_TEMPLATE: &str = "2TffvPucqHkN";

/// Invites made from "Invite People" last this long, as on the web client.
pub const INVITE_MAX_AGE: u32 = 7 * 24 * 60 * 60;

/// `POST /invites/{code}`; from a card under a message, `instance` names
/// that message and code as the web client does.
pub fn join(session: Option<String>, instance: Option<&str>) -> Value {
    let mut body = json!({ "session_id": session });
    if let Some(instance) = instance {
        body["invite_instance_id"] = json!(instance);
    }
    body
}

/// `invite_instance_id` of a join from the card under `message`.
pub fn invite_instance(message: Snowflake, code: &str) -> String {
    format!("{message}:{code}")
}

/// The `X-Context-Properties` of a join: where the invite leads, and the
/// card's message when it came from one.
pub fn join_context(invite: &Invite, from: Option<(Snowflake, &str)>) -> Value {
    let mut context = json!({
        "location": if from.is_some() { "Invite Button Embed" } else { "Join Guild" },
        "location_guild_id": invite.guild.as_ref().map(|g| g.id.to_string()),
        "location_channel_id": invite.channel.as_ref().map(|c| c.id.to_string()),
        "location_channel_type": invite.channel.as_ref().map(|c| c.kind),
    });
    if let Some((message, instance)) = from {
        context["location_message_id"] = json!(message.to_string());
        context["invite_instance_id"] = json!(instance);
    }
    context
}

/// "Invite People": the modal's first invite, with its defaults (seven
/// days, no use limit, not temporary).
pub fn create_invite() -> Value {
    json!({
        "validate": null,
        "max_age": INVITE_MAX_AGE,
        "max_uses": 0,
        "temporary": false,
        "flags": 0,
    })
}

/// Leaving a server the user is a full member of.
pub fn leave_guild() -> Value {
    json!({ "lurking": false })
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
