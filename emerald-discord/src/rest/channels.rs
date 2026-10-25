//! Private channels: group DMs and their members, closing a DM, and
//! message requests.

use anyhow::Result;
use reqwest::StatusCode;

use super::{DiscordCall, Rest};
use crate::model::{Channel, Snowflake};
use crate::packets::{context, user};
use emerald_http::{Call, Solution};

impl Rest {
    /// A group DM with `users`, as "New Group DM" starts one.
    pub async fn create_group(&self, users: &[Snowflake], captcha: Option<&Solution>) -> Result<Channel> {
        self.api
            .fetch(
                Call::post("/users/@me/channels", "/users/@me/channels")
                    .json(user::open_dm(users))
                    .context(context::location("New Group DM"))
                    .captcha(captcha),
            )
            .await
    }

    /// Adds `recipient` to `channel`. Adding someone to a 1:1 DM makes a
    /// new group, which Discord answers with (201); a group just grows.
    pub async fn add_recipient(&self, channel: Snowflake, recipient: Snowflake) -> Result<Option<Channel>> {
        let call = Call::put("/channels/{id}/recipients/{user}", format!("/channels/{channel}/recipients/{recipient}"))
            .major(channel)
            .context(context::location("Add Friends to DM"));
        let response = self.api.send(&call).await?;
        if response.status() == StatusCode::CREATED {
            return Ok(Some(response.json().await?));
        }
        Ok(None)
    }

    /// Removes someone from a group DM the user owns.
    pub async fn remove_recipient(&self, channel: Snowflake, recipient: Snowflake) -> Result<()> {
        let path = format!("/channels/{channel}/recipients/{recipient}");
        self.api.run(Call::delete("/channels/{id}/recipients/{user}", path).major(channel)).await
    }

    /// Renames a group DM or gives it a new icon; `body` says which.
    pub async fn edit_group(&self, channel: Snowflake, body: serde_json::Value) -> Result<()> {
        self.api
            .run(Call::patch("/channels/{id}", format!("/channels/{channel}")).major(channel).json(body))
            .await
    }

    /// Closes a DM or leaves a group DM, telling the group as usual.
    pub async fn close_channel(&self, channel: Snowflake) -> Result<()> {
        self.api
            .run(Call::delete("/channels/{id}", format!("/channels/{channel}?silent=false")).major(channel))
            .await
    }

    /// Accepts a message request, or takes a DM out of the spam inbox.
    pub async fn accept_request(&self, channel: Snowflake) -> Result<()> {
        self.api
            .run(
                Call::put("/channels/{id}/recipients/@me", format!("/channels/{channel}/recipients/@me"))
                    .major(channel)
                    .json(user::accept_request()),
            )
            .await
    }

    /// Ignores a message request, which removes it.
    pub async fn reject_request(&self, channel: Snowflake) -> Result<()> {
        let path = format!("/channels/{channel}/recipients/@me");
        self.api.run(Call::delete("/channels/{id}/recipients/@me", path).major(channel)).await
    }
}
