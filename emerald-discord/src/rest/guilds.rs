//! Servers: joining through an invite, creating one, inviting people and
//! leaving.

use anyhow::Result;

use super::{DiscordCall, Rest};
use crate::model::{Invite, Snowflake};
use crate::packets::{context, guild};
use emerald_http::{Call, Policy, Solution};

impl Rest {
    /// An invite as its card shows it, with the counts and expiry the web
    /// client asks for.
    pub async fn invite(&self, code: &str) -> Result<Invite> {
        self.api
            .fetch(
                Call::get("/invites/{code}", format!("/invites/{code}?with_counts=true&with_expiration=true"))
                    .cached(Policy::MICRO),
            )
            .await
    }

    /// Resolves the invite first, like the web client, so the join carries
    /// the guild and channel it came from; `message` is the message whose
    /// invite card was clicked, if any.
    pub async fn join_guild(&self, code: &str, message: Option<Snowflake>, captcha: Option<&Solution>) -> Result<()> {
        let invite = self.invite(code).await?;
        let instance = message.map(|message| guild::invite_instance(message, code));
        let from = message.zip(instance.as_deref());
        self.api.run(
            Call::post("/invites/{code}", format!("/invites/{code}"))
                .json(guild::join(self.identity.session_id(), instance.as_deref()))
                .context(guild::join_context(&invite, from))
                .captcha(captcha),
        )
        .await
    }

    pub async fn create_guild(&self, name: &str, captcha: Option<&Solution>) -> Result<()> {
        self.api.run(Call::post("/guilds", "/guilds").json(guild::create(name)).captcha(captcha)).await
    }

    /// "Invite People" on `channel`; `location` is the surface it came from.
    pub async fn create_invite(&self, channel: Snowflake, location: &str) -> Result<Invite> {
        self.api
            .fetch(
                Call::post("/channels/{id}/invites", format!("/channels/{channel}/invites"))
                    .major(channel)
                    .json(guild::create_invite())
                    .context(context::location(location)),
            )
            .await
    }

    /// "Leave Server"; the gateway answers with GUILD_DELETE.
    pub async fn leave_guild(&self, guild: Snowflake) -> Result<()> {
        self.api
            .run(
                Call::delete("/users/@me/guilds/{id}", format!("/users/@me/guilds/{guild}"))
                    .major(guild)
                    .json(guild::leave_guild()),
            )
            .await
    }
}
