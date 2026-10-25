//! Interactions: what the message box sends to run an app's slash command.

use serde_json::{Map, Value};

use crate::model::Snowflake;

/// An interaction of type 2 (application command), with the body the web
/// client's command box sends.
pub fn slash_command(
    application: Snowflake,
    guild: Option<Snowflake>,
    channel: Snowflake,
    session: Option<String>,
    data: Value,
    nonce: &str,
) -> Value {
    let mut body = Map::new();
    body.insert("type".into(), 2.into());
    body.insert("application_id".into(), application.to_string().into());
    if let Some(guild) = guild {
        body.insert("guild_id".into(), guild.to_string().into());
    }
    body.insert("channel_id".into(), channel.to_string().into());
    body.insert("session_id".into(), session.into());
    body.insert("data".into(), data);
    body.insert("nonce".into(), nonce.into());
    body.insert("analytics_location".into(), "slash_ui".into());
    body.into()
}
