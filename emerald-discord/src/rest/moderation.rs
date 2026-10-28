//! Moderation and channel settings: deleting many messages, kicking,
//! banning, timing out, and saving a channel or thread.

use anyhow::Result;

use super::{DiscordCall, Rest};
use crate::model::Snowflake;
use crate::packets::moderation;
use crate::protocol::ChannelEdit;
use emerald_http::{Call, Solution};

impl Rest {
    /// Deletes `messages` of `channel` at once, from the message menu's
    /// selection. The web client's route; not seen in its bundle, so the
    /// path is a guess from the public API.
    pub async fn bulk_delete(&self, channel: Snowflake, messages: &[Snowflake], captcha: Option<&Solution>) -> Result<()> {
        self.api
            .run(
                Call::post("/channels/{id}/messages/bulk-delete", format!("/channels/{channel}/messages/bulk-delete"))
                    .major(channel)
                    .json(moderation::bulk_delete(messages))
                    .captcha(captcha),
            )
            .await
    }

    /// "Kick": the member leaves the server until they join again.
    pub async fn kick(&self, guild: Snowflake, user: Snowflake, captcha: Option<&Solution>) -> Result<()> {
        self.api
            .run(
                Call::delete("/guilds/{id}/members/{user}", format!("/guilds/{guild}/members/{user}"))
                    .major(guild)
                    .captcha(captcha),
            )
            .await
    }

    /// "Ban": `delete_seconds` of their messages are deleted with them.
    pub async fn ban(&self, guild: Snowflake, user: Snowflake, delete_seconds: u32, captcha: Option<&Solution>) -> Result<()> {
        self.api
            .run(
                Call::put("/guilds/{id}/bans/{user}", format!("/guilds/{guild}/bans/{user}"))
                    .major(guild)
                    .json(moderation::ban(delete_seconds))
                    .captcha(captcha),
            )
            .await
    }

    /// Times a member out until `until`, an ISO timestamp, or lifts it.
    pub async fn timeout(&self, guild: Snowflake, user: Snowflake, until: Option<&str>) -> Result<()> {
        self.api
            .run(
                Call::patch("/guilds/{id}/members/{user}", format!("/guilds/{guild}/members/{user}"))
                    .major(guild)
                    .json(moderation::timeout(until)),
            )
            .await
    }

    /// Saves a channel's or thread's settings with `PATCH /channels/{id}`.
    pub async fn edit_channel(&self, channel: Snowflake, edit: &ChannelEdit) -> Result<()> {
        self.api
            .run(
                Call::patch("/channels/{id}", format!("/channels/{channel}"))
                    .major(channel)
                    .json(moderation::channel(edit)),
            )
            .await
    }
}
