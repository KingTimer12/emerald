//! The account and its relationships.

use anyhow::Result;

use super::{DiscordCall, PROFILE_CACHE, Rest};
use crate::model::{Channel, Profile, Snowflake, User};
use crate::packets::{context, user};
use emerald_http::{Call, Solution};

impl Rest {
    /// The profile behind the DM side panel, with the same query the web
    /// client sends for a DM.
    pub async fn profile(&self, user: Snowflake, guild: Option<Snowflake>) -> Result<Profile> {
        let mut path = format!(
            "/users/{user}/profile?with_mutual_guilds=false&with_mutual_friends=false&with_mutual_friends_count=false"
        );
        if let Some(guild) = guild {
            path.push_str(&format!("&guild_id={guild}"));
        }
        self.api.fetch(Call::get("/users/{id}/profile", path).cached(PROFILE_CACHE)).await
    }

    /// Revokes the token server-side, as the web client does on "Log Out".
    pub async fn logout(&self) -> Result<()> {
        self.api.run(Call::post("/auth/logout", "/auth/logout").json(user::logout()))
            .await
    }

    pub async fn send_friend_request(&self, username: &str, captcha: Option<&Solution>) -> Result<()> {
        self.api.run(
            Call::post("/users/@me/relationships", "/users/@me/relationships")
                .json(user::friend_request(username))
                .context(context::location("Add Friend"))
                .captcha(captcha),
        )
        .await
    }

    pub async fn accept_friend(&self, user: Snowflake) -> Result<()> {
        self.api.run(
            Call::put("/users/@me/relationships/{id}", format!("/users/@me/relationships/{user}"))
                .json(user::accept_friend())
                .context(context::location("Friends")),
        )
        .await
    }

    /// Removes a friend, declines an incoming request or cancels an outgoing one.
    pub async fn remove_relationship(&self, user: Snowflake) -> Result<()> {
        self.api.run(
            Call::delete("/users/@me/relationships/{id}", format!("/users/@me/relationships/{user}"))
                .context(context::location("Friends")),
        )
        .await
    }

    pub async fn open_dm(&self, user: Snowflake, captcha: Option<&Solution>) -> Result<Channel> {
        self.api.fetch(
            Call::post("/users/@me/channels", "/users/@me/channels")
                .json(user::open_dm(user))
                .context(context::empty())
                .captcha(captcha),
        )
        .await
    }

    /// `None` clears the display name back to the username.
    pub async fn set_global_name(&self, name: Option<&str>) -> Result<User> {
        self.api.fetch(Call::patch("/users/@me", "/users/@me").json(user::global_name(name)))
            .await
    }
}
