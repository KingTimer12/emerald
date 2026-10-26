//! The account's own changes: the synced settings PATCH (a partial
//! `PreloadedUserSettings`), the profile and member PATCHes, and the
//! custom status as a presence activity.

use serde_json::{Map, Value, json};

use super::user::data_uri;
use crate::model::proto::{Writer, encode_base64};
use crate::model::user_settings::field;
use crate::model::{CustomStatus, Snowflake, UserSettings};
use crate::protocol::{Picture, ProfileEdit, SettingsEdit};

/// `PATCH /users/@me/settings-proto/1`: the section `edit` touches, rewritten
/// whole from `current` (Discord replaces each section it receives), based
/// on the version the edit was made against. `now_ms` dates a new custom
/// status. `None` when a stored section cannot be read back.
pub fn settings_edit(current: &UserSettings, edit: &SettingsEdit, now_ms: u64) -> Option<Value> {
    let (number, section) = match edit {
        SettingsEdit::Status(status) => {
            let mut section = Writer::keep(current.status.as_deref().unwrap_or_default(), &[field::STATUS_NAME])?;
            section.wrapped_string(field::STATUS_NAME, status.as_str());
            (field::STATUS, section)
        }
        SettingsEdit::CustomStatus(custom) => {
            let mut section = Writer::keep(current.status.as_deref().unwrap_or_default(), &[field::CUSTOM_STATUS])?;
            if let Some(custom) = custom {
                section.message(field::CUSTOM_STATUS, custom_status(custom, now_ms));
            }
            (field::STATUS, section)
        }
        SettingsEdit::DmsRestricted { restricted, existing } => privacy(
            current,
            field::DEFAULT_GUILDS_RESTRICTED_V2,
            *restricted,
            field::RESTRICTED_GUILD_IDS,
            existing.as_deref(),
        )?,
        SettingsEdit::MessageRequestsRestricted { restricted, existing } => privacy(
            current,
            field::DEFAULT_MESSAGE_REQUEST_RESTRICTED,
            *restricted,
            field::MESSAGE_REQUEST_RESTRICTED_GUILD_IDS,
            existing.as_deref(),
        )?,
        SettingsEdit::FriendSources(flags) => {
            let mut section =
                Writer::keep(current.privacy.as_deref().unwrap_or_default(), &[field::FRIEND_SOURCE_FLAGS])?;
            section.wrapped_varint(field::FRIEND_SOURCE_FLAGS, u64::from(*flags));
            (field::PRIVACY, section)
        }
    };
    let mut settings = Writer::default();
    settings.message(number, section);
    Some(json!({
        "settings": encode_base64(&settings.into_bytes()),
        "required_data_version": current.data_version(),
    }))
}

fn custom_status(custom: &CustomStatus, now_ms: u64) -> Writer {
    let mut message = Writer::default();
    if !custom.text.is_empty() {
        message.bytes(field::TEXT, custom.text.as_bytes());
    }
    if let Some(id) = custom.emoji_id {
        message.fixed64(field::EMOJI_ID, id.0);
    }
    if let Some(name) = &custom.emoji_name {
        message.bytes(field::EMOJI_NAME, name.as_bytes());
    }
    if custom.expires_at_ms != 0 {
        message.fixed64(field::EXPIRES_AT_MS, custom.expires_at_ms);
    }
    message.fixed64(field::CREATED_AT_MS, now_ms);
    message
}

/// The privacy section with a default flag changed and, when `existing`
/// is given, its list of servers replaced.
fn privacy(
    current: &UserSettings,
    flag: u32,
    on: bool,
    list: u32,
    existing: Option<&[Snowflake]>,
) -> Option<(u32, Writer)> {
    let replaced: &[u32] = if existing.is_some() { &[flag, list] } else { &[flag] };
    let mut section = Writer::keep(current.privacy.as_deref().unwrap_or_default(), replaced)?;
    section.wrapped_varint(flag, u64::from(on));
    if let Some(guilds) = existing.filter(|g| !g.is_empty()) {
        let ids: Vec<u64> = guilds.iter().map(|g| g.0).collect();
        section.packed_fixed64(list, &ids);
    }
    Some((field::PRIVACY, section))
}

/// `PATCH /users/@me` (global) or `/guilds/{id}/members/@me` (a server):
/// the name and avatar, in the web client's field order.
pub fn user_edit(edit: &ProfileEdit, guild: bool) -> Value {
    let mut body = Map::new();
    if guild && let Some(name) = &edit.name {
        body.insert("nick".into(), json!(name));
    }
    if let Some(avatar) = &edit.avatar {
        body.insert("avatar".into(), picture(avatar.as_ref()));
    }
    if !guild && let Some(name) = &edit.name {
        body.insert("global_name".into(), json!(name));
    }
    Value::Object(body)
}

/// `PATCH /users/@me/profile` or `/guilds/{id}/profile/@me`.
pub fn profile_edit(edit: &ProfileEdit) -> Value {
    let mut body = Map::new();
    if let Some(bio) = &edit.bio {
        body.insert("bio".into(), json!(bio));
    }
    if let Some(pronouns) = &edit.pronouns {
        body.insert("pronouns".into(), json!(pronouns));
    }
    if let Some(color) = &edit.accent_color {
        body.insert("accent_color".into(), json!(color));
    }
    if let Some(banner) = &edit.banner {
        body.insert("banner".into(), picture(banner.as_ref()));
    }
    Value::Object(body)
}

fn picture(picture: Option<&Picture>) -> Value {
    picture.map_or(Value::Null, |(mime, bytes)| Value::String(data_uri(mime, bytes)))
}

/// The custom status as the activity the web client puts in its presence.
pub fn custom_status_activity(custom: &CustomStatus) -> Value {
    let mut activity = Map::new();
    activity.insert("name".into(), json!("Custom Status"));
    activity.insert("type".into(), json!(crate::model::activity_kind::CUSTOM));
    if !custom.text.is_empty() {
        activity.insert("state".into(), json!(custom.text));
    }
    if custom.expires_at_ms != 0 {
        activity.insert("timestamps".into(), json!({ "end": custom.expires_at_ms }));
    }
    let emoji = match (&custom.emoji_id, &custom.emoji_name) {
        (Some(id), Some(name)) => json!({ "id": id.to_string(), "name": name, "animated": false }),
        (None, Some(name)) => json!({ "id": null, "name": name, "animated": false }),
        _ => Value::Null,
    };
    activity.insert("emoji".into(), emoji);
    activity.insert("metadata".into(), json!({}));
    Value::Object(activity)
}

/// `POST /auth/sessions/logout`.
pub fn log_out_sessions(hashes: &[String]) -> Value {
    json!({ "session_id_hashes": hashes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Status;

    fn decoded(body: &Value) -> UserSettings {
        UserSettings::decode(body["settings"].as_str().unwrap()).unwrap()
    }

    #[test]
    fn edits_send_their_whole_section() {
        let current = UserSettings::default();
        let custom = CustomStatus {
            text: "ocupado".into(),
            emoji_name: Some("🔥".into()),
            expires_at_ms: 5000,
            ..Default::default()
        };
        let body = settings_edit(&current, &SettingsEdit::CustomStatus(Some(custom.clone())), 1000).unwrap();
        assert_eq!(body["required_data_version"], 0);
        let sent = decoded(&body);
        assert_eq!(sent.custom_status(), Some(custom));
        assert_eq!(sent.privacy, None);

        // Picking a status keeps the custom status beside it.
        let status = decoded(&settings_edit(&sent, &SettingsEdit::Status(Status::Dnd), 0).unwrap());
        assert_eq!(status.status(), Some(Status::Dnd));
        assert_eq!(status.custom_status().unwrap().text, "ocupado");
        let cleared = settings_edit(&sent, &SettingsEdit::CustomStatus(None), 0).unwrap();
        assert_eq!(decoded(&cleared).custom_status(), None);
    }

    #[test]
    fn privacy_edits_keep_the_other_choices() {
        let friends = settings_edit(&UserSettings::default(), &SettingsEdit::FriendSources(2), 0).unwrap();
        let current = decoded(&friends);
        let dms = SettingsEdit::DmsRestricted {
            restricted: true,
            existing: Some(vec![Snowflake(1)]),
        };
        let privacy = decoded(&settings_edit(&current, &dms, 0).unwrap()).privacy();
        assert!(privacy.dms_restricted);
        assert_eq!(privacy.friend_sources, 2);
    }

    #[test]
    fn profile_bodies_hold_only_what_changed() {
        let edit = ProfileEdit {
            name: Some(None),
            pronouns: Some("ela/dela".into()),
            ..Default::default()
        };
        assert_eq!(user_edit(&edit, false), json!({ "global_name": null }));
        assert_eq!(user_edit(&edit, true), json!({ "nick": null }));
        assert_eq!(profile_edit(&edit), json!({ "pronouns": "ela/dela" }));
    }

    #[test]
    fn custom_status_activity_matches_the_web_client() {
        let custom = CustomStatus {
            text: "oi".into(),
            ..Default::default()
        };
        assert_eq!(
            custom_status_activity(&custom).to_string(),
            r#"{"name":"Custom Status","type":4,"state":"oi","emoji":null,"metadata":{}}"#
        );
    }
}
