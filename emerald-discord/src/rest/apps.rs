//! Apps: their slash commands, and running one.

use anyhow::Result;
use reqwest::StatusCode;

use super::Rest;
use crate::model::{CommandIndex, Snowflake};
use crate::packets::interaction;
use crate::protocol::IndexTarget;
use emerald_http::Call;

impl Rest {
    /// The slash commands of `target`. `None` while Discord is still
    /// building the index (202), to ask again later.
    pub async fn command_index(&self, target: IndexTarget) -> Result<Option<CommandIndex>> {
        let call = match target {
            IndexTarget::Guild(guild) => Call::get(
                "/guilds/{id}/application-command-index",
                format!("/guilds/{guild}/application-command-index"),
            )
            .major(guild),
            IndexTarget::Channel(channel) => Call::get(
                "/channels/{id}/application-command-index",
                format!("/channels/{channel}/application-command-index"),
            )
            .major(channel),
            IndexTarget::User => {
                Call::get("/users/@me/application-command-index", "/users/@me/application-command-index")
            }
        };
        let response = self.api.send(&call).await?;
        if response.status() == StatusCode::ACCEPTED {
            return Ok(None);
        }
        Ok(Some(response.json().await?))
    }

    /// Runs a slash command: an interaction of type 2, with the body the
    /// web client's command box sends.
    pub async fn run_command(
        &self,
        application: Snowflake,
        guild: Option<Snowflake>,
        channel: Snowflake,
        data: serde_json::Value,
        nonce: &str,
    ) -> Result<()> {
        let body = interaction::slash_command(application, guild, channel, self.identity.session_id(), data, nonce);
        self.api.run(Call::post("/interactions", "/interactions").json(body)).await
    }
}
