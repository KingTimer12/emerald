//! The account's own settings and profile, and the devices signed in to it.

use anyhow::Result;
use serde::Deserialize;

use super::Rest;
use crate::model::{AuthSession, AuthSessions, Member, Snowflake, User, UserProfileDetails, UserSettings};
use crate::packets::account;
use crate::protocol::{ProfileEdit, SettingsEdit};
use emerald_http::Call;

/// What a settings PATCH answers: the whole settings as saved.
#[derive(Deserialize)]
struct SavedSettings {
    settings: String,
    /// The edit was based on an older version and was not saved.
    #[serde(default)]
    out_of_date: bool,
}

/// The settings as Discord holds them after an edit, and whether the edit
/// was dropped as out of date.
pub struct SettingsSaved {
    pub settings: Option<UserSettings>,
    pub out_of_date: bool,
}

impl Rest {
    pub async fn edit_settings(&self, current: &UserSettings, edit: &SettingsEdit, now_ms: u64) -> Result<SettingsSaved> {
        let body = account::settings_edit(current, edit, now_ms)
            .ok_or_else(|| anyhow::anyhow!("stored settings could not be read back"))?;
        let saved: SavedSettings = self
            .api
            .fetch(Call::patch("/users/@me/settings-proto/{type}", "/users/@me/settings-proto/1").json(body))
            .await?;
        Ok(SettingsSaved {
            settings: UserSettings::decode(&saved.settings),
            out_of_date: saved.out_of_date,
        })
    }

    /// The display name and avatar.
    pub async fn edit_user(&self, edit: &ProfileEdit) -> Result<User> {
        self.api.fetch(Call::patch("/users/@me", "/users/@me").json(account::user_edit(edit, false))).await
    }

    /// The nickname and avatar in `guild`; the gateway echoes the member.
    pub async fn edit_member(&self, guild: Snowflake, edit: &ProfileEdit) -> Result<Member> {
        self.api
            .fetch(
                Call::patch("/guilds/{id}/members/@me", format!("/guilds/{guild}/members/@me"))
                    .major(guild)
                    .json(account::user_edit(edit, true)),
            )
            .await
    }

    /// Bio, pronouns, banner and accent color, globally or in `guild`.
    pub async fn edit_profile(&self, guild: Option<Snowflake>, edit: &ProfileEdit) -> Result<UserProfileDetails> {
        let call = match guild {
            Some(guild) => Call::patch("/guilds/{id}/profile/@me", format!("/guilds/{guild}/profile/@me")).major(guild),
            None => Call::patch("/users/@me/profile", "/users/@me/profile"),
        };
        self.api.fetch(call.json(account::profile_edit(edit))).await
    }

    pub async fn sessions(&self) -> Result<Vec<AuthSession>> {
        let sessions: AuthSessions = self.api.fetch(Call::get("/auth/sessions", "/auth/sessions")).await?;
        Ok(sessions.user_sessions)
    }

    pub async fn log_out_sessions(&self, hashes: &[String]) -> Result<()> {
        self.api
            .run(Call::post("/auth/sessions/logout", "/auth/sessions/logout").json(account::log_out_sessions(hashes)))
            .await
    }
}
