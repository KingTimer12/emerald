//! Servers: joining through an invite and creating one.

use anyhow::Result;

use super::{DiscordCall, Rest};
use crate::model::Invite;
use crate::packets::guild;
use emerald_http::{Call, Policy, Solution};

impl Rest {
    /// Resolves the invite first, like the web client, so the join carries
    /// the guild and channel it came from.
    pub async fn join_guild(&self, code: &str, captcha: Option<&Solution>) -> Result<()> {
        let invite: Invite = self
            .api
            .fetch(
                Call::get("/invites/{code}", format!("/invites/{code}?with_counts=true&with_expiration=true"))
                    .cached(Policy::MICRO),
            )
            .await?;
        self.api.run(
            Call::post("/invites/{code}", format!("/invites/{code}"))
                .json(guild::join(self.identity.session_id()))
                .context(guild::join_context(&invite))
                .captcha(captcha),
        )
        .await
    }

    pub async fn create_guild(&self, name: &str, captcha: Option<&Solution>) -> Result<()> {
        self.api.run(Call::post("/guilds", "/guilds").json(guild::create(name)).captcha(captcha)).await
    }
}
