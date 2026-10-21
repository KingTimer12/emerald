//! REST client for the Discord API and CDN. Every call goes through
//! [`RateLimiter`], GETs that opt in go through the [`microcache`], and
//! errors come back as the typed failures in [`error`].

mod error;
mod microcache;
mod ratelimit;

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, bail};
use reqwest::header::{
    ACCEPT, ACCEPT_LANGUAGE, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue, ORIGIN, REFERER, USER_AGENT,
};
use reqwest::{Method, Response, StatusCode};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::json;
use tokio::time::Instant;

use base64::Engine;

use bytes::Bytes;
pub use error::{CaptchaRequired, Challenge, Forbidden, Solution, Unauthorized};
use error::{ErrorBody, Unavailable};
use microcache::{Lookup, MicroCache, Policy, Ticket};
use ratelimit::{RateLimiter, Route};

use super::{ReactionSource, ReplyTo, Upload};
use super::cache::Cache;
use super::identity::{self, Identity};
use super::model::{Channel, DISCORD_EPOCH_MS, Emoji, Invite, Message, Profile, Snowflake, User};

const MAX_ATTEMPTS: u32 = 4;
pub const HISTORY_PAGE: usize = 50;

/// Template the web client uses for "Create My Own".
const CREATE_GUILD_TEMPLATE: &str = "2TffvPucqHkN";
/// Major parameter for routes that are not scoped to a channel or guild.
const NO_MAJOR: Snowflake = Snowflake(0);
/// Profiles: the microcache's second, a stale copy for ten minutes while a
/// refresh is slow or failing, and the last one on disk after a restart.
const PROFILE_CACHE: Policy = Policy {
    fresh: Duration::from_secs(1),
    stale: Duration::from_secs(600),
    persist: true,
};

#[derive(Deserialize)]
struct RateLimited {
    retry_after: f64,
}

/// One API request, built up step by step. The route template names its
/// rate-limit bucket and `major` (channel or guild) scopes it.
struct Call<'a> {
    route: Route,
    major: Snowflake,
    path: String,
    body: Option<serde_json::Value>,
    context: Option<String>,
    captcha: Option<&'a Solution>,
    cache: Option<Policy>,
}

impl<'a> Call<'a> {
    fn new(method: Method, route: &'static str, path: impl Into<String>) -> Self {
        Self {
            route: (method, route),
            major: NO_MAJOR,
            path: path.into(),
            body: None,
            context: None,
            captcha: None,
            cache: None,
        }
    }

    fn get(route: &'static str, path: impl Into<String>) -> Self {
        Self::new(Method::GET, route, path)
    }

    fn post(route: &'static str, path: impl Into<String>) -> Self {
        Self::new(Method::POST, route, path)
    }

    fn put(route: &'static str, path: impl Into<String>) -> Self {
        Self::new(Method::PUT, route, path)
    }

    fn patch(route: &'static str, path: impl Into<String>) -> Self {
        Self::new(Method::PATCH, route, path)
    }

    fn delete(route: &'static str, path: impl Into<String>) -> Self {
        Self::new(Method::DELETE, route, path)
    }

    /// Scopes the bucket to a channel or guild, as Discord does.
    fn major(mut self, major: Snowflake) -> Self {
        self.major = major;
        self
    }

    /// Serves a GET from the microcache under `policy`.
    fn cached(mut self, policy: Policy) -> Self {
        debug_assert_eq!(self.route.0, Method::GET);
        self.cache = Some(policy);
        self
    }

    fn json(mut self, body: serde_json::Value) -> Self {
        self.body = Some(body);
        self
    }

    /// `X-Context-Properties`: base64 JSON telling Discord which UI surface
    /// triggered the request, as the web client sends on relationship and
    /// invite calls.
    fn context(mut self, properties: serde_json::Value) -> Self {
        self.context = Some(base64::engine::general_purpose::STANDARD.encode(properties.to_string()));
        self
    }

    fn captcha(mut self, captcha: Option<&'a Solution>) -> Self {
        self.captcha = captcha;
        self
    }
}

pub struct Rest {
    http: reqwest::Client,
    /// Plain client for cdn.discordapp.com: no API headers, no auth.
    cdn: reqwest::Client,
    /// Puts attachments to the storage URLs Discord hands out; never sends
    /// the token, as the browser's cross-origin upload does not.
    uploads: reqwest::Client,
    /// CDN images and kept responses on disk; `None` runs uncached.
    cache: Option<Arc<Cache>>,
    micro: MicroCache,
    identity: Arc<Identity>,
    api: String,
    limits: RateLimiter,
}

impl Rest {
    pub fn new(identity: Arc<Identity>, cache: Option<Arc<Cache>>) -> Result<Self> {
        // Same header set the web client sends on its API calls.
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("*/*"));
        headers.insert(REFERER, HeaderValue::from_static("https://discord.com/channels/@me"));
        headers.insert("sec-ch-ua", HeaderValue::from_static(identity::SEC_CH_UA));
        headers.insert("sec-ch-ua-mobile", HeaderValue::from_static("?0"));
        headers.insert("sec-ch-ua-platform", HeaderValue::from_static(identity::SEC_CH_UA_PLATFORM));
        headers.insert("sec-fetch-dest", HeaderValue::from_static("empty"));
        headers.insert("sec-fetch-mode", HeaderValue::from_static("cors"));
        headers.insert("sec-fetch-site", HeaderValue::from_static("same-origin"));
        headers.insert("x-debug-options", HeaderValue::from_static("bugReporterEnabled"));
        headers.insert("x-discord-timezone", HeaderValue::from_str(&identity.timezone)?);
        headers.insert("x-super-properties", HeaderValue::from_str(&identity.super_properties)?);
        headers.insert(USER_AGENT, HeaderValue::from_static(identity::USER_AGENT));

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .pool_max_idle_per_host(1)
            .pool_idle_timeout(Duration::from_secs(60))
            .timeout(Duration::from_secs(20))
            .build()?;
        let cdn = reqwest::Client::builder()
            .user_agent(identity::USER_AGENT)
            .pool_max_idle_per_host(2)
            .timeout(Duration::from_secs(20))
            .build()?;
        let mut upload_headers = HeaderMap::new();
        upload_headers.insert(ORIGIN, HeaderValue::from_static("https://discord.com"));
        upload_headers.insert(REFERER, HeaderValue::from_static("https://discord.com/"));
        upload_headers.insert("sec-ch-ua", HeaderValue::from_static(identity::SEC_CH_UA));
        upload_headers.insert("sec-ch-ua-mobile", HeaderValue::from_static("?0"));
        upload_headers.insert("sec-ch-ua-platform", HeaderValue::from_static(identity::SEC_CH_UA_PLATFORM));
        upload_headers.insert("sec-fetch-dest", HeaderValue::from_static("empty"));
        upload_headers.insert("sec-fetch-mode", HeaderValue::from_static("cors"));
        upload_headers.insert("sec-fetch-site", HeaderValue::from_static("cross-site"));
        let uploads = reqwest::Client::builder()
            .default_headers(upload_headers)
            .user_agent(identity::USER_AGENT)
            .pool_max_idle_per_host(1)
            // Big files on slow links take long; only a stalled one fails.
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(120))
            .build()?;
        Ok(Self {
            http,
            cdn,
            uploads,
            cache,
            micro: MicroCache::default(),
            api: format!("https://discord.com/api/v{}", identity::API_VERSION),
            identity,
            limits: RateLimiter::default(),
        })
    }

    /// Fetches a CDN image from the disk cache, or downloads and caches it.
    /// Refuses anything implausibly large for an avatar or sticker.
    pub async fn cdn_image(&self, url: &str) -> Result<Vec<u8>> {
        if let Some(bytes) = self.on_disk(|cache, url| cache.image(url), url).await.flatten() {
            return Ok(bytes);
        }
        let bytes = self.download(url).await?;
        let stored = bytes.clone();
        self.on_disk(move |cache, url| cache.put_image(url, &stored), url).await;
        Ok(bytes)
    }

    /// Runs `op` on the disk cache off the async thread; `None` when there
    /// is no cache.
    async fn on_disk<T: Send + 'static>(
        &self,
        op: impl FnOnce(&Cache, &str) -> T + Send + 'static,
        key: &str,
    ) -> Option<T> {
        let cache = self.cache.clone()?;
        let key = key.to_owned();
        tokio::task::spawn_blocking(move || op(&cache, &key)).await.ok()
    }

    async fn download(&self, url: &str) -> Result<Vec<u8>> {
        const MAX_BYTES: usize = 2 * 1024 * 1024;
        let response = self.cdn.get(url).send().await?.error_for_status()?;
        if response.content_length().is_some_and(|len| len as usize > MAX_BYTES) {
            bail!(t!("errors.image_too_large"));
        }
        let bytes = response.bytes().await?;
        if bytes.len() > MAX_BYTES {
            bail!(t!("errors.image_too_large"));
        }
        Ok(bytes.to_vec())
    }

    pub async fn messages(
        &self,
        channel: Snowflake,
        before: Option<Snowflake>,
    ) -> Result<Vec<Message>> {
        let mut path = format!("/channels/{channel}/messages?limit={HISTORY_PAGE}");
        if let Some(before) = before {
            path.push_str(&format!("&before={before}"));
        }
        let mut messages: Vec<Message> = self
            .fetch(Call::get("/channels/{id}/messages", path).major(channel).cached(Policy::MICRO))
            .await?;
        messages.reverse();
        Ok(messages)
    }

    /// The first message of each forum post in `threads`, at most 10 at a
    /// time as the web client asks.
    pub async fn forum_posts(&self, forum: Snowflake, threads: &[Snowflake]) -> Result<Vec<(Snowflake, Message)>> {
        let ids: Vec<String> = threads.iter().map(ToString::to_string).collect();
        let data: PostData = self
            .fetch(
                Call::post("/channels/{id}/post-data", format!("/channels/{forum}/post-data"))
                    .major(forum)
                    .json(json!({ "thread_ids": ids })),
            )
            .await?;
        Ok(data
            .threads
            .into_iter()
            .filter_map(|(id, post)| Some((Snowflake(id.parse().ok()?), post.first_message?)))
            .collect())
    }

    /// A page of a forum's archived posts, newest activity first.
    pub async fn archived_posts(&self, forum: Snowflake, offset: usize) -> Result<ThreadSearch> {
        let path = format!(
            "/channels/{forum}/threads/search?archived=true&sort_by=last_message_time&sort_order=desc\
             &limit={FORUM_PAGE}&tag_setting=match_some&offset={offset}"
        );
        self.fetch(Call::get("/channels/{id}/threads/search", path).major(forum).cached(Policy::MICRO))
            .await
    }

    /// `nonce` comes back on the MESSAGE_CREATE echo, which is how the UI
    /// swaps its pending copy for the real message.
    pub async fn send_message(
        &self,
        channel: Snowflake,
        content: &str,
        nonce: &str,
        reply: Option<ReplyTo>,
        files: &[Uploaded],
        captcha: Option<&Solution>,
    ) -> Result<Message> {
        let mut body = json!({
            "mobile_network_type": "unknown",
            "content": content,
            "nonce": nonce,
            "tts": false,
            "flags": 0,
        });
        if !files.is_empty() {
            // Ids are the files' places in this list, as the web client sends.
            let attachments: Vec<_> = files
                .iter()
                .enumerate()
                .map(|(ix, file)| {
                    let mut attachment = json!({
                        "id": ix.to_string(),
                        "filename": file.filename,
                        "uploaded_filename": file.uploaded_filename,
                    });
                    // Both are left out unless set, as the web client does.
                    if !file.description.is_empty() {
                        attachment["description"] = file.description.clone().into();
                    }
                    if file.spoiler {
                        attachment["is_spoiler"] = true.into();
                    }
                    attachment
                })
                .collect();
            body["attachments"] = attachments.into();
        }
        if let Some(reply) = reply {
            body["message_reference"] = json!({
                "channel_id": channel.to_string(),
                "message_id": reply.message.to_string(),
            });
            // Like the web client: other mentions still ping, the author not.
            if !reply.mention {
                body["allowed_mentions"] = json!({
                    "parse": ["users", "roles", "everyone"],
                    "replied_user": false,
                });
            }
        }
        self.fetch(
            Call::post("/channels/{id}/messages", format!("/channels/{channel}/messages"))
                .major(channel)
                .json(body)
                .captcha(captcha),
        )
        .await
    }

    /// Puts each file in Discord's storage, as the web client does: ask the
    /// channel for an upload URL, then PUT the bytes there. The message then
    /// names the files by their `uploaded_filename`.
    pub async fn upload_files(&self, channel: Snowflake, files: &[Upload]) -> Result<Vec<Uploaded>> {
        let mut uploaded = Vec::with_capacity(files.len());
        for file in files {
            let mut entry = json!({
                "filename": file.name,
                "file_size": file.bytes.len(),
                "id": upload_id(),
                "is_clip": false,
            });
            if let Some(mime) = mime_type(&file.name) {
                entry["original_content_type"] = mime.into();
            }
            let answer: UploadUrls = self
                .fetch(
                    Call::post("/channels/{id}/attachments", format!("/channels/{channel}/attachments"))
                        .major(channel)
                        .json(json!({ "files": [entry] })),
                )
                .await?;
            let Some(target) = answer.attachments.into_iter().next() else {
                bail!("Discord gave no upload URL for {}", file.name);
            };
            if !target.upload_url.starts_with("https://") {
                bail!("refusing a non-HTTPS upload URL for {}", file.name);
            }
            let response = self
                .uploads
                .put(&target.upload_url)
                .header(CONTENT_TYPE, "application/octet-stream")
                .body(reqwest::Body::from(bytes::Bytes::from_owner(file.bytes.clone())))
                .send()
                .await?;
            if !response.status().is_success() {
                bail!("uploading {} failed: {}", file.name, response.status());
            }
            uploaded.push(Uploaded {
                filename: file.name.clone(),
                uploaded_filename: target.upload_filename,
                description: file.description.clone(),
                spoiler: file.spoiler,
            });
        }
        Ok(uploaded)
    }

    pub async fn edit_message(&self, channel: Snowflake, message: Snowflake, content: &str) -> Result<()> {
        self.run(
            Call::patch("/channels/{id}/messages/{id}", format!("/channels/{channel}/messages/{message}"))
                .major(channel)
                .json(json!({ "content": content })),
        )
        .await
    }

    pub async fn delete_message(&self, channel: Snowflake, message: Snowflake) -> Result<()> {
        self.run(
            Call::delete("/channels/{id}/messages/{id}", format!("/channels/{channel}/messages/{message}"))
                .major(channel),
        )
        .await
    }

    pub async fn pin_message(&self, channel: Snowflake, message: Snowflake, pin: bool) -> Result<()> {
        let path = format!("/channels/{channel}/messages/pins/{message}");
        let call = if pin {
            Call::put("/channels/{id}/messages/pins/{id}", path)
        } else {
            Call::delete("/channels/{id}/messages/pins/{id}", path)
        };
        self.run(call.major(channel)).await
    }

    /// Our normal reaction, as the web client adds it: `type=0` and the
    /// surface it came from as `location`.
    pub async fn add_reaction(
        &self,
        channel: Snowflake,
        message: Snowflake,
        emoji: &Emoji,
        source: ReactionSource,
    ) -> Result<()> {
        let path = format!(
            "/channels/{channel}/messages/{message}/reactions/{}/@me?location={}&type=0",
            encode_path(&emoji.route_key()),
            encode_component(source.location()),
        );
        self.run(Call::put("/channels/{id}/messages/{id}/reactions/{emoji}/@me", path).major(channel))
            .await
    }

    /// Removes our normal reaction through the typed route the web client uses.
    pub async fn remove_reaction(
        &self,
        channel: Snowflake,
        message: Snowflake,
        emoji: &Emoji,
        source: ReactionSource,
    ) -> Result<()> {
        let path = format!(
            "/channels/{channel}/messages/{message}/reactions/{}/0/@me?location={}&burst=false",
            encode_path(&emoji.route_key()),
            encode_component(source.location()),
        );
        self.run(Call::delete("/channels/{id}/messages/{id}/reactions/{emoji}/{type}/@me", path).major(channel))
            .await
    }

    /// Read receipt, as the web client sends when a channel is on screen.
    pub async fn ack(&self, channel: Snowflake, message: Snowflake) -> Result<()> {
        self.run(
            Call::post(
                "/channels/{id}/messages/{id}/ack",
                format!("/channels/{channel}/messages/{message}/ack"),
            )
            .major(channel)
            .json(json!({ "token": null })),
        )
        .await
    }

    /// Shows "… is typing" to the other side for about ten seconds.
    pub async fn typing(&self, channel: Snowflake) -> Result<()> {
        self.run(Call::post("/channels/{id}/typing", format!("/channels/{channel}/typing")).major(channel))
            .await
    }

    /// The profile behind the DM side panel, with the same query the web
    /// client sends for a DM.
    pub async fn profile(&self, user: Snowflake, guild: Option<Snowflake>) -> Result<Profile> {
        let mut path = format!(
            "/users/{user}/profile?with_mutual_guilds=false&with_mutual_friends=false&with_mutual_friends_count=false"
        );
        if let Some(guild) = guild {
            path.push_str(&format!("&guild_id={guild}"));
        }
        self.fetch(Call::get("/users/{id}/profile", path).cached(PROFILE_CACHE)).await
    }

    /// Revokes the token server-side, as the web client does on "Log Out".
    pub async fn logout(&self) -> Result<()> {
        self.run(Call::post("/auth/logout", "/auth/logout").json(json!({ "provider": null, "voip_provider": null })))
            .await
    }

    pub async fn send_friend_request(&self, username: &str, captcha: Option<&Solution>) -> Result<()> {
        self.run(
            Call::post("/users/@me/relationships", "/users/@me/relationships")
                .json(json!({ "username": username, "discriminator": null }))
                .context(json!({ "location": "Add Friend" }))
                .captcha(captcha),
        )
        .await
    }

    pub async fn accept_friend(&self, user: Snowflake) -> Result<()> {
        self.run(
            Call::put("/users/@me/relationships/{id}", format!("/users/@me/relationships/{user}"))
                .json(json!({}))
                .context(json!({ "location": "Friends" })),
        )
        .await
    }

    /// Removes a friend, declines an incoming request or cancels an outgoing one.
    pub async fn remove_relationship(&self, user: Snowflake) -> Result<()> {
        self.run(
            Call::delete("/users/@me/relationships/{id}", format!("/users/@me/relationships/{user}"))
                .context(json!({ "location": "Friends" })),
        )
        .await
    }

    pub async fn open_dm(&self, user: Snowflake, captcha: Option<&Solution>) -> Result<Channel> {
        self.fetch(
            Call::post("/users/@me/channels", "/users/@me/channels")
                .json(json!({ "recipients": [user.to_string()] }))
                .context(json!({}))
                .captcha(captcha),
        )
        .await
    }

    /// Resolves the invite first, like the web client, so the join carries
    /// the guild and channel it came from.
    pub async fn join_guild(&self, code: &str, captcha: Option<&Solution>) -> Result<()> {
        let invite: Invite = self
            .fetch(
                Call::get("/invites/{code}", format!("/invites/{code}?with_counts=true&with_expiration=true"))
                    .cached(Policy::MICRO),
            )
            .await?;
        let properties = json!({
            "location": "Join Guild",
            "location_guild_id": invite.guild.as_ref().map(|g| g.id.to_string()),
            "location_channel_id": invite.channel.as_ref().map(|c| c.id.to_string()),
            "location_channel_type": invite.channel.as_ref().map(|c| c.kind),
        });
        self.run(
            Call::post("/invites/{code}", format!("/invites/{code}"))
                .json(json!({ "session_id": self.identity.session_id() }))
                .context(properties)
                .captcha(captcha),
        )
        .await
    }

    pub async fn create_guild(&self, name: &str, captcha: Option<&Solution>) -> Result<()> {
        let body = json!({
            "name": name,
            "icon": null,
            "channels": [],
            "system_channel_id": null,
            "guild_template_code": CREATE_GUILD_TEMPLATE,
        });
        self.run(Call::post("/guilds", "/guilds").json(body).captcha(captcha)).await
    }

    /// `None` clears the display name back to the username.
    pub async fn set_global_name(&self, name: Option<&str>) -> Result<User> {
        self.fetch(Call::patch("/users/@me", "/users/@me").json(json!({ "global_name": name })))
            .await
    }

    /// Sends `call` and decodes the JSON answer.
    async fn fetch<T: DeserializeOwned>(&self, call: Call<'_>) -> Result<T> {
        match call.cache {
            Some(policy) => Ok(serde_json::from_slice(&self.cached(&call, policy).await?)?),
            None => Ok(self.send(&call).await?.json().await?),
        }
    }

    /// Sends `call` for its effect, ignoring the body.
    async fn run(&self, call: Call<'_>) -> Result<()> {
        self.send(&call).await.map(drop)
    }

    /// The answer to a GET, from the microcache when it has a usable copy.
    async fn cached(&self, call: &Call<'_>, policy: Policy) -> Result<Bytes> {
        loop {
            match self.micro.lookup(&call.path, policy, call.major) {
                Lookup::Hit(body) => return Ok(body),
                Lookup::Wait(mut inflight) => {
                    if let Ok(answer) = inflight.wait_for(Option::is_some).await
                        && let Some(answer) = answer.clone()
                    {
                        return answer.map_err(anyhow::Error::msg);
                    }
                    // That request was cancelled: look again, and fetch if no one else is.
                }
                Lookup::Fetch(ticket) => return self.refresh(call, policy, ticket).await,
            }
        }
    }

    async fn refresh(&self, call: &Call<'_>, policy: Policy, ticket: Ticket<'_>) -> Result<Bytes> {
        let result = async { Ok::<_, anyhow::Error>(self.send(call).await?.bytes().await?) }.await;
        let error = match result {
            Ok(body) => {
                if policy.persist {
                    let (account, kept) = (self.identity.account(), body.clone());
                    self.on_disk(move |cache, key| cache.put_response(account, key, &kept), ticket.key())
                        .await;
                }
                ticket.finish(body.clone());
                return Ok(body);
            }
            Err(error) if is_transient(&error) => error,
            Err(error) => {
                ticket.share(Err(error.to_string()));
                return Err(error);
            }
        };
        let mut stand_in = ticket.stale().cloned();
        if stand_in.is_none() && policy.persist {
            let account = self.identity.account();
            let kept = self.on_disk(move |cache, key| cache.response(account, key), ticket.key()).await;
            stand_in = kept.flatten().map(|(body, _)| body.into());
        }
        match stand_in {
            Some(body) => {
                log::info!("{} failed ({error}); serving a cached copy", call.route.1);
                ticket.share(Ok(body.clone()));
                Ok(body)
            }
            None => {
                ticket.share(Err(error.to_string()));
                Err(error)
            }
        }
    }

    /// Sends `call` within the rate limits, retrying after 429s.
    async fn send(&self, call: &Call<'_>) -> Result<Response> {
        let route = &call.route;
        for _ in 0..MAX_ATTEMPTS {
            let mut bucket = self.limits.acquire(route, call.major).await?;

            // Sensitive: kept out of debug output and never HPACK-indexed.
            let mut authorization = HeaderValue::from_str(self.identity.token().expose())?;
            authorization.set_sensitive(true);
            let mut request = self
                .http
                .request(route.0.clone(), format!("{}{}", self.api, call.path))
                .header(AUTHORIZATION, authorization)
                .header(ACCEPT_LANGUAGE, &*self.identity.accept_language())
                .header("x-discord-locale", &*self.identity.locale());
            if let Some(body) = &call.body {
                request = request.header(ORIGIN, "https://discord.com").json(body);
            }
            if let Some(context) = &call.context {
                request = request.header("x-context-properties", context);
            }
            if let Some(captcha) = call.captcha {
                request = request.header("x-captcha-key", &captcha.key);
                if let Some(rqtoken) = &captcha.rqtoken {
                    request = request.header("x-captcha-rqtoken", rqtoken);
                }
                if let Some(session_id) = &captcha.session_id {
                    request = request.header("x-captcha-session-id", session_id);
                }
            }
            let response = request.send().await?;
            self.limits.observe(route, &mut bucket, response.headers());

            match response.status() {
                StatusCode::TOO_MANY_REQUESTS => {
                    let global = response.headers().contains_key("x-ratelimit-global");
                    let retry_after = response
                        .json::<RateLimited>()
                        .await
                        .map_or(1.0, |r| r.retry_after);
                    let until = Instant::now() + Duration::from_secs_f64(retry_after.max(0.0));
                    log::warn!("429 on {} {}, retry in {retry_after}s (global: {global})", route.0, route.1);
                    self.limits.note_invalid();
                    if global {
                        self.limits.block_global(until);
                    } else {
                        bucket.block(until);
                    }
                }
                StatusCode::UNAUTHORIZED => {
                    self.limits.note_invalid();
                    bail!(Unauthorized);
                }
                StatusCode::FORBIDDEN => {
                    self.limits.note_invalid();
                    bail!(Forbidden(ErrorBody::read(response).await.message()));
                }
                status if status.is_success() => {
                    // A write makes what was read from the same channel or guild stale.
                    if route.0 != Method::GET && call.major != NO_MAJOR {
                        self.micro.invalidate(call.major);
                    }
                    return Ok(response);
                }
                status if status.is_server_error() => {
                    bail!(Unavailable(ErrorBody::read(response).await.into_error(status).to_string()))
                }
                status => return Err(ErrorBody::read(response).await.into_error(status)),
            }
        }
        bail!(Unavailable(t!("errors.rate_limited", route = format!("{} {}", route.0, route.1)).to_owned()))
    }
}

/// Failures worth answering from a stale copy: Discord or the network could
/// not answer, as opposed to answering no.
fn is_transient(error: &anyhow::Error) -> bool {
    error.is::<Unavailable>() || error.is::<reqwest::Error>()
}

/// Snowflake-shaped nonce, like the web client's. The low bits carry a
/// counter so two sends in the same millisecond still differ.
pub fn nonce() -> String {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    let count = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed) & 0x3f_ffff;
    (ms.saturating_sub(DISCORD_EPOCH_MS) << 22 | count).to_string()
}

/// Percent-encodes a path segment the way the browser's URL parser does
/// when the web client puts raw text in a path: non-ASCII and the path
/// percent-encode set, while `:` stays as is.
fn encode_path(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_graphic() && !b"\"#<>?`{}/%".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Percent-encodes a query value as `encodeURIComponent` does, which is
/// how the web client's HTTP library writes query strings.
fn encode_component(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Extracts the invite code from what a user pastes: a bare code,
/// `discord.gg/<code>`, or `discord.com/invite/<code>`, with or without
/// scheme, trailing slash or query string.
pub fn invite_code(input: &str) -> Option<String> {
    let input = input.trim();
    let input = input.split(['?', '#']).next().unwrap_or_default();
    let code = input.trim_end_matches('/').rsplit('/').next()?;
    let valid = !code.is_empty()
        && code.len() <= 64
        && code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    valid.then(|| code.to_owned())
}

/// Archived forum posts per page, as the web client loads them.
pub const FORUM_PAGE: usize = 25;

#[derive(Deserialize)]
struct PostData {
    #[serde(default)]
    threads: std::collections::HashMap<String, Post>,
}

#[derive(Deserialize)]
struct Post {
    #[serde(default)]
    first_message: Option<Message>,
}

#[derive(Deserialize)]
pub struct ThreadSearch {
    #[serde(default)]
    pub threads: Vec<Channel>,
    /// Our memberships among `threads`.
    #[serde(default)]
    pub members: Vec<super::model::ThreadMemberEntry>,
    #[serde(default)]
    pub first_messages: Vec<Message>,
    #[serde(default)]
    pub has_more: bool,
}

/// A file already in Discord's storage, ready to name in a message.
pub struct Uploaded {
    filename: String,
    uploaded_filename: String,
    description: String,
    spoiler: bool,
}

#[derive(Deserialize)]
struct UploadUrls {
    attachments: Vec<UploadUrl>,
}

#[derive(Deserialize)]
struct UploadUrl {
    upload_url: String,
    upload_filename: String,
}

/// The web client numbers upload requests with lodash's `uniqueId`: a
/// counter shared with the rest of the page, sent as a string.
fn upload_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed).to_string()
}

/// The type a browser reports for a file, from its extension; unknown
/// ones send none.
fn mime_type(name: &str) -> Option<&'static str> {
    let extension = name.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "wav" => "audio/wav",
        "txt" => "text/plain",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "json" => "application/json",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::{encode_component, encode_path, invite_code};

    #[test]
    fn reaction_routes_encode_like_the_browser() {
        assert_eq!(encode_path("👍"), "%F0%9F%91%8D");
        assert_eq!(encode_path("pepe:42"), "pepe:42");
        assert_eq!(encode_component("Message Hover Bar"), "Message%20Hover%20Bar");
    }

    #[test]
    fn invite_code_accepts_pasted_forms() {
        for input in [
            "abc-DEF12",
            " https://discord.gg/abc-DEF12 ",
            "discord.gg/abc-DEF12/",
            "https://discord.com/invite/abc-DEF12?event=1",
        ] {
            assert_eq!(invite_code(input).as_deref(), Some("abc-DEF12"), "{input}");
        }
    }

    #[test]
    fn invite_code_rejects_junk() {
        assert_eq!(invite_code(""), None);
        assert_eq!(invite_code("https://discord.gg/"), None);
        assert_eq!(invite_code("not a code"), None);
    }
}
