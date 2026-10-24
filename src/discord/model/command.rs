//! Application ("slash") commands, as the web client's command index lists
//! them. Each command keeps the JSON it came in, since running one sends
//! the whole command back as `application_command`.

use serde::Deserialize;
use serde_json::Value;

use super::Snowflake;
use super::snowflake::bits;

/// `/name` commands typed in the message box; the others are menu items.
pub const CHAT_INPUT: u8 = 1;

pub mod option_kind {
    pub const SUB_COMMAND: u8 = 1;
    pub const SUB_COMMAND_GROUP: u8 = 2;
    pub const STRING: u8 = 3;
    pub const INTEGER: u8 = 4;
    pub const BOOLEAN: u8 = 5;
    pub const USER: u8 = 6;
    pub const CHANNEL: u8 = 7;
    pub const ROLE: u8 = 8;
    pub const MENTIONABLE: u8 = 9;
    pub const NUMBER: u8 = 10;
    pub const ATTACHMENT: u8 = 11;
}

/// `GET …/application-command-index`.
#[derive(Debug, Default, Deserialize)]
pub struct CommandIndex {
    #[serde(default)]
    pub applications: Vec<CommandApplication>,
    #[serde(default)]
    pub application_commands: Vec<Value>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CommandApplication {
    pub id: Snowflake,
    #[serde(default)]
    pub name: String,
}

/// A command the box can run.
#[derive(Clone, Debug)]
pub struct AppCommand {
    pub id: Snowflake,
    pub application_id: Snowflake,
    pub version: String,
    pub kind: u8,
    pub name: String,
    pub description: String,
    pub options: Vec<CommandOption>,
    /// Set for commands made for one guild.
    pub guild_id: Option<Snowflake>,
    /// Permissions a member needs, unless an admin changed them.
    pub default_member_permissions: Option<u64>,
    /// The command as Discord sent it, sent back when it runs.
    pub raw: Value,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CommandOption {
    #[serde(rename = "type")]
    pub kind: u8,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub choices: Vec<CommandChoice>,
    /// A subcommand's own options.
    #[serde(default)]
    pub options: Vec<CommandOption>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CommandChoice {
    pub name: String,
    pub value: Value,
}

#[derive(Deserialize)]
struct Fields {
    id: Snowflake,
    application_id: Snowflake,
    #[serde(default)]
    version: String,
    #[serde(rename = "type", default = "chat_input")]
    kind: u8,
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    options: Vec<CommandOption>,
    #[serde(default)]
    guild_id: Option<Snowflake>,
    #[serde(default, deserialize_with = "optional_bits")]
    default_member_permissions: Option<u64>,
}

fn chat_input() -> u8 {
    CHAT_INPUT
}

fn optional_bits<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<u64>, D::Error> {
    #[derive(Deserialize)]
    struct Bits(#[serde(deserialize_with = "bits")] u64);
    Ok(Option::<Bits>::deserialize(deserializer)?.map(|b| b.0))
}

impl AppCommand {
    /// The command in `raw`, or `None` when it does not parse.
    pub fn parse(raw: Value) -> Option<Self> {
        let fields = Fields::deserialize(&raw).ok()?;
        Some(Self {
            id: fields.id,
            application_id: fields.application_id,
            version: fields.version,
            kind: fields.kind,
            name: fields.name,
            description: fields.description,
            options: fields.options,
            guild_id: fields.guild_id,
            default_member_permissions: fields.default_member_permissions,
            raw,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_keep_their_json() {
        let raw = serde_json::json!({
            "id": "10", "application_id": "20", "version": "30", "type": 1, "name": "roll",
            "description": "Rolls dice", "default_member_permissions": "8",
            "options": [{ "type": 4, "name": "sides", "description": "", "required": true }],
        });
        let command = AppCommand::parse(raw.clone()).unwrap();
        assert_eq!((command.name.as_str(), command.kind), ("roll", CHAT_INPUT));
        assert_eq!(command.default_member_permissions, Some(8));
        assert!(command.options[0].required);
        assert_eq!(command.raw, raw);
        let index: CommandIndex =
            serde_json::from_str(r#"{"applications":[{"id":"20","name":"Dice"}],"application_commands":[]}"#).unwrap();
        assert_eq!(index.applications[0].name, "Dice");
    }
}
