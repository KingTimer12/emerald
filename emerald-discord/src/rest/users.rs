//! The account and its relationships: friends, blocks, nicknames, notes
//! and profiles.

use anyhow::Result;

use super::{DiscordCall, PROFILE_CACHE, Rest};
use crate::model::{Channel, CollectiblesProduct, NoteUpdate, Profile, Snowflake, User};
use crate::packets::{context, user};
use emerald_http::{Call, Refused, Solution};

impl Rest {
    /// The profile behind the DM side panel, with the same query the web
    /// client sends for a DM; `full` asks as the full profile modal does,
    /// for the servers in common and how many friends are.
    pub async fn profile(&self, user: Snowflake, guild: Option<Snowflake>, full: bool) -> Result<Profile> {
        let mut path = if full {
            format!(
                "/users/{user}/profile?type=modal&with_mutual_guilds=true&with_mutual_friends=false&with_mutual_friends_count=true"
            )
        } else {
            format!(
                "/users/{user}/profile?with_mutual_guilds=false&with_mutual_friends=false&with_mutual_friends_count=false"
            )
        };
        if let Some(guild) = guild {
            path.push_str(&format!("&guild_id={guild}"));
        }
        self.api.fetch(Call::get("/users/{id}/profile", path).cached(PROFILE_CACHE)).await
    }

    /// A shop product by SKU, as the web client asks for the profile
    /// effect a profile shows right after fetching it.
    pub async fn collectibles_product(&self, sku: &str) -> Result<CollectiblesProduct> {
        let path = format!("/collectibles-products/{sku}?locale={}", self.identity.locale());
        self.api
            .fetch(Call::get("/collectibles-products/{sku}", path).cached(PROFILE_CACHE))
            .await
    }

    /// The friends the user shares with `user`.
    pub async fn mutual_friends(&self, user: Snowflake) -> Result<Vec<User>> {
        self.api
            .fetch(Call::get("/users/{id}/relationships", format!("/users/{user}/relationships")))
            .await
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

    /// Removes a friend, declines an incoming request, cancels an outgoing
    /// one or unblocks, from the surface `location` names.
    pub async fn remove_relationship(&self, user: Snowflake, location: &str) -> Result<()> {
        self.api.run(
            Call::delete("/users/@me/relationships/{id}", format!("/users/@me/relationships/{user}"))
                .context(context::location(location)),
        )
        .await
    }

    /// "Block" from a user's context menu.
    pub async fn block(&self, user: Snowflake) -> Result<()> {
        self.api.run(
            Call::put("/users/@me/relationships/{id}", format!("/users/@me/relationships/{user}"))
                .json(user::block())
                .context(context::location("ContextMenu")),
        )
        .await
    }

    /// "Ignore", or "Unignore" with `ignore` false.
    pub async fn ignore(&self, user: Snowflake, ignore: bool) -> Result<()> {
        let route = "/users/@me/relationships/{id}/ignore";
        let path = format!("/users/@me/relationships/{user}/ignore");
        let call = if ignore { Call::put(route, path) } else { Call::delete(route, path) };
        self.api.run(call.context(context::location("ContextMenu"))).await
    }

    /// A friend's nickname; `None` clears it.
    pub async fn set_nickname(&self, user: Snowflake, nickname: Option<&str>) -> Result<()> {
        self.api.run(
            Call::patch("/users/@me/relationships/{id}", format!("/users/@me/relationships/{user}"))
                .json(user::nickname(nickname)),
        )
        .await
    }

    /// The user's note about `user`; Discord answers 404 when there is none.
    pub async fn note(&self, user: Snowflake) -> Result<String> {
        let call = Call::get("/users/@me/notes/{id}", format!("/users/@me/notes/{user}"));
        match self.api.fetch::<NoteUpdate>(call).await {
            Ok(found) => Ok(if found.id == user { found.note } else { String::new() }),
            Err(e) if e.downcast_ref::<Refused>().is_some_and(|r| r.status.as_u16() == 404) => Ok(String::new()),
            Err(e) => Err(e),
        }
    }

    /// Empty clears the note.
    pub async fn set_note(&self, user: Snowflake, note: &str) -> Result<()> {
        self.api
            .run(Call::put("/users/@me/notes/{id}", format!("/users/@me/notes/{user}")).json(user::note(note)))
            .await
    }

    pub async fn open_dm(&self, user: Snowflake, captcha: Option<&Solution>) -> Result<Channel> {
        self.api.fetch(
            Call::post("/users/@me/channels", "/users/@me/channels")
                .json(user::open_dm(&[user]))
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
