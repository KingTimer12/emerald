//! Running a slash command typed in the message box. The box is plain text,
//! so options are written as `name:value` after the command (and its
//! subcommand words), the way Discord shows them; a first string option
//! may also be typed bare, as in `/say hello`.

use serde_json::{Value, json};

use emerald_discord::model::{AppCommand, CommandOption, option_kind};

/// Why a typed command cannot run.
#[derive(Debug, PartialEq, Eq)]
pub enum CommandError {
    /// A subcommand is needed; these are the ones there are.
    Subcommand(Vec<String>),
    Missing(String),
    /// The option's value does not fit its kind or its choices.
    Invalid(String),
    /// Files cannot be given from the box yet.
    Unsupported(String),
}

/// What follows `/name` in `text`, if `text` runs `command`.
pub fn arguments<'a>(command: &AppCommand, text: &'a str) -> Option<&'a str> {
    let rest = text.trim_start().strip_prefix('/')?.strip_prefix(command.name.as_str())?;
    (rest.is_empty() || rest.starts_with(char::is_whitespace)).then_some(rest)
}

/// The interaction's `data` for `command` run with `args`, as the web
/// client builds it.
pub fn interaction_data(command: &AppCommand, args: &str) -> Result<Value, CommandError> {
    let mut options = &command.options;
    let mut path: Vec<(u8, &str)> = Vec::new();
    let mut rest = args.trim();
    while options.iter().any(is_subcommand) {
        let (word, after) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        let Some(sub) = options.iter().find(|o| is_subcommand(o) && o.name.eq_ignore_ascii_case(word)) else {
            let names = options.iter().filter(|o| is_subcommand(o)).map(|o| o.name.clone()).collect();
            return Err(CommandError::Subcommand(names));
        };
        path.push((sub.kind, sub.name.as_str()));
        options = &sub.options;
        rest = after.trim();
    }

    let given = split_options(options, rest);
    let mut values = Vec::new();
    for option in options {
        let Some(text) = given.iter().find(|(name, _)| *name == option.name).map(|(_, text)| *text) else {
            if option.required {
                return Err(CommandError::Missing(option.name.clone()));
            }
            continue;
        };
        values.push(json!({ "type": option.kind, "name": option.name, "value": value(option, text)? }));
    }
    let mut built = Value::Array(values);
    for (kind, name) in path.into_iter().rev() {
        built = json!([{ "type": kind, "name": name, "options": built }]);
    }

    let mut data = json!({
        "version": command.version,
        "id": command.id.to_string(),
        "name": command.name,
        "type": command.kind,
        "options": built,
        "application_command": command.raw,
    });
    if let Some(guild) = command.guild_id {
        data["guild_id"] = guild.to_string().into();
    }
    Ok(data)
}

fn is_subcommand(option: &CommandOption) -> bool {
    matches!(option.kind, option_kind::SUB_COMMAND | option_kind::SUB_COMMAND_GROUP)
}

/// Each `name:` of `options` found in `text` with the text up to the next
/// one. Text before the first goes to the first string option, if any.
fn split_options<'a>(options: &'a [CommandOption], text: &'a str) -> Vec<(&'a str, &'a str)> {
    let mut marks: Vec<(usize, usize, &str)> = Vec::new();
    for option in options {
        let label = format!("{}:", option.name);
        let mut from = 0;
        while let Some(found) = text[from..].find(&label) {
            let at = from + found;
            let starts_word = text[..at].chars().next_back().is_none_or(char::is_whitespace);
            if starts_word {
                marks.push((at, at + label.len(), option.name.as_str()));
                break;
            }
            from = at + label.len();
        }
    }
    marks.sort_unstable();
    let mut found = Vec::new();
    let leading = text[..marks.first().map_or(text.len(), |m| m.0)].trim();
    if !leading.is_empty()
        && let Some(first) = options.iter().find(|o| o.kind == option_kind::STRING)
        && !marks.iter().any(|m| m.2 == first.name)
    {
        found.push((first.name.as_str(), leading));
    }
    for (ix, (_, value_start, name)) in marks.iter().enumerate() {
        let end = marks.get(ix + 1).map_or(text.len(), |next| next.0);
        found.push((*name, text[*value_start..end].trim()));
    }
    found
}

/// `text` as the JSON value `option` takes.
fn value(option: &CommandOption, text: &str) -> Result<Value, CommandError> {
    let invalid = || CommandError::Invalid(option.name.clone());
    if !option.choices.is_empty() {
        return option
            .choices
            .iter()
            .find(|c| {
                c.name.eq_ignore_ascii_case(text)
                    || c.value.as_str().is_some_and(|v| v.eq_ignore_ascii_case(text))
                    || c.value.as_f64().is_some_and(|v| text.parse::<f64>() == Ok(v))
            })
            .map(|c| c.value.clone())
            .ok_or_else(invalid);
    }
    match option.kind {
        option_kind::STRING if !text.is_empty() => Ok(text.into()),
        option_kind::INTEGER => text.parse::<i64>().map(Into::into).map_err(|_| invalid()),
        option_kind::NUMBER => text.parse::<f64>().ok().and_then(serde_json::Number::from_f64).map(Value::Number).ok_or_else(invalid),
        option_kind::BOOLEAN => match text.to_ascii_lowercase().as_str() {
            "true" | "yes" => Ok(true.into()),
            "false" | "no" => Ok(false.into()),
            _ => Err(invalid()),
        },
        option_kind::USER | option_kind::CHANNEL | option_kind::ROLE | option_kind::MENTIONABLE => {
            mentioned_id(text).map(Into::into).ok_or_else(invalid)
        }
        option_kind::ATTACHMENT => Err(CommandError::Unsupported(option.name.clone())),
        _ => Err(invalid()),
    }
}

/// The id in `<@1>`, `<@!1>`, `<@&1>`, `<#1>`, or a bare id.
fn mentioned_id(text: &str) -> Option<String> {
    let inner = text
        .strip_prefix('<')
        .and_then(|t| t.strip_suffix('>'))
        .map(|t| t.trim_start_matches(['@', '#', '!', '&']))
        .unwrap_or(text);
    (!inner.is_empty() && inner.bytes().all(|b| b.is_ascii_digit())).then(|| inner.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(options: Value) -> AppCommand {
        AppCommand::parse(json!({
            "id": "1", "application_id": "2", "version": "3", "type": 1, "name": "game",
            "description": "", "options": options,
        }))
        .unwrap()
    }

    #[test]
    fn options_are_read_by_name() {
        let cmd = command(json!([
            { "type": 3, "name": "text", "required": true },
            { "type": 4, "name": "times" },
            { "type": 6, "name": "who" },
        ]));
        assert_eq!(arguments(&cmd, "/game text:hi there times:2"), Some(" text:hi there times:2"));
        assert_eq!(arguments(&cmd, "/gamer"), None);
        let data = interaction_data(&cmd, "text:hi there times:2 who:<@55>").unwrap();
        assert_eq!(
            data["options"],
            json!([
                { "type": 3, "name": "text", "value": "hi there" },
                { "type": 4, "name": "times", "value": 2 },
                { "type": 6, "name": "who", "value": "55" },
            ])
        );
        assert_eq!(data["application_command"]["name"], "game");
        assert_eq!(data["id"], "1");
        // A bare first string.
        let data = interaction_data(&cmd, "hello").unwrap();
        assert_eq!(data["options"][0]["value"], "hello");
        assert_eq!(interaction_data(&cmd, ""), Err(CommandError::Missing("text".into())));
        assert_eq!(interaction_data(&cmd, "text:a times:x"), Err(CommandError::Invalid("times".into())));
    }

    #[test]
    fn subcommands_and_choices() {
        let cmd = command(json!([
            { "type": 1, "name": "start", "options": [
                { "type": 3, "name": "mode", "required": true, "choices": [
                    { "name": "Hard", "value": "hard" }, { "name": "Easy", "value": "easy" },
                ] },
            ] },
            { "type": 1, "name": "stop" },
        ]));
        let data = interaction_data(&cmd, "start mode:hard").unwrap();
        assert_eq!(
            data["options"],
            json!([{ "type": 1, "name": "start", "options": [{ "type": 3, "name": "mode", "value": "hard" }] }])
        );
        assert_eq!(interaction_data(&cmd, "stop").unwrap()["options"][0]["options"], json!([]));
        assert_eq!(
            interaction_data(&cmd, "pause"),
            Err(CommandError::Subcommand(vec!["start".into(), "stop".into()]))
        );
        assert_eq!(interaction_data(&cmd, "start mode:medium"), Err(CommandError::Invalid("mode".into())));
    }
}
