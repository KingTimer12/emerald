//! Notification settings and read states.

use std::time::Duration;

use anyhow::Result;

use super::Rest;
use crate::model::Snowflake;
use crate::packets::settings;
use crate::protocol::SettingsChange;
use emerald_http::Call;

/// Channels per bulk receipt, and the pause between receipts, as the web
/// client's "Mark As Read" sends them.
const ACK_CHUNK: usize = 100;
const ACK_GAP: Duration = Duration::from_secs(1);

impl Rest {
    /// Saves `change` for `guild`, or for the DMs when it is `None`. The
    /// gateway echoes the new settings as USER_GUILD_SETTINGS_UPDATE.
    pub async fn guild_settings(&self, guild: Option<Snowflake>, change: SettingsChange) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);
        let body = settings::guild_settings(change, now);
        let call = match guild {
            Some(guild) => Call::patch("/users/@me/guilds/settings", "/users/@me/guilds/settings")
                .json(settings::bulk(guild, body)),
            None => Call::patch("/users/@me/guilds/{id}/settings", "/users/@me/guilds/@me/settings").json(body),
        };
        self.api.run(call).await
    }

    /// Marks each channel read up to its message, a hundred at a time.
    pub async fn ack_bulk(&self, channels: &[(Snowflake, Snowflake)]) -> Result<()> {
        for (ix, chunk) in channels.chunks(ACK_CHUNK).enumerate() {
            if ix > 0 {
                tokio::time::sleep(ACK_GAP).await;
            }
            self.api.run(Call::post("/read-states/ack-bulk", "/read-states/ack-bulk").json(settings::ack_bulk(chunk)))
                .await?;
        }
        Ok(())
    }
}
