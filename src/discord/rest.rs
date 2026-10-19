//! REST client with Discord's rate-limit contract:
//! - per-bucket limits from `X-RateLimit-*` headers, requests to one bucket
//!   are serialized so `remaining` is never overshot;
//! - a conservative global cap below Discord's 50 req/s;
//! - 429 handling via `retry_after` (bucket or global);
//! - an invalid-request guard (401/403/429) far below the 10k/10min
//!   Cloudflare ban threshold.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Result, bail};
use reqwest::header::{
    ACCEPT, ACCEPT_LANGUAGE, AUTHORIZATION, HeaderMap, HeaderValue, ORIGIN, REFERER, USER_AGENT,
};
use reqwest::{Method, Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::time::{Instant, sleep, sleep_until};

use base64::Engine;

use super::identity::{self, Identity};
use super::model::{Channel, Invite, Message, Snowflake, User};

const GLOBAL_PER_SECOND: u32 = 25;
const INVALID_WINDOW: Duration = Duration::from_secs(600);
const INVALID_CAP: u32 = 500;
const MAX_ATTEMPTS: u32 = 4;
const MAX_BUCKETS: usize = 256;
pub const HISTORY_PAGE: usize = 50;
/// Discord epoch, used to build message nonces.
const DISCORD_EPOCH_MS: u64 = 1_420_070_400_000;

/// 403, with Discord's (localized) reason when the body has one.
#[derive(Debug)]
pub struct Forbidden(pub Option<String>);

impl std::fmt::Display for Forbidden {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_deref().unwrap_or("sem permissão"))
    }
}

impl std::error::Error for Forbidden {}

/// 401: the token was revoked or expired; the session must sign in again.
#[derive(Debug)]
pub struct Unauthorized;

impl std::fmt::Display for Unauthorized {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("401: token inválido ou expirado")
    }
}

impl std::error::Error for Unauthorized {}

/// hCaptcha challenge from a 400 response. The user solves it in a webview
/// (`webview::solve_captcha`); nothing here solves it automatically.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Challenge {
    pub sitekey: String,
    pub rqdata: Option<String>,
    pub rqtoken: Option<String>,
    pub session_id: Option<String>,
}

/// A solved challenge, sent back as the `X-Captcha-*` headers on the retry.
pub struct Solution {
    pub key: String,
    pub rqtoken: Option<String>,
    pub session_id: Option<String>,
}

impl Solution {
    pub fn new(key: String, challenge: &Challenge) -> Self {
        Self {
            key,
            rqtoken: challenge.rqtoken.clone(),
            session_id: challenge.session_id.clone(),
        }
    }
}

/// Discord asked for a captcha. Shown to the user only when they close the
/// captcha window without solving it.
#[derive(Debug)]
pub struct CaptchaRequired(pub Challenge);

impl std::fmt::Display for CaptchaRequired {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("A verificação (captcha) não foi concluída.")
    }
}

impl std::error::Error for CaptchaRequired {}

/// Error body Discord returns on 4xx. `message` is localized through
/// `X-Discord-Locale`, so it is shown to the user as is.
#[derive(Default, Deserialize)]
struct ErrorBody {
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    captcha_key: Option<serde::de::IgnoredAny>,
    #[serde(default)]
    captcha_sitekey: Option<String>,
    #[serde(default)]
    captcha_rqdata: Option<String>,
    #[serde(default)]
    captcha_rqtoken: Option<String>,
    #[serde(default)]
    captcha_session_id: Option<String>,
}

impl ErrorBody {
    async fn read(response: Response) -> Self {
        response.json().await.unwrap_or_default()
    }
}

/// `X-Context-Properties`: base64 JSON telling Discord which UI surface
/// triggered the request, as the web client sends on relationship and
/// invite calls.
fn context(properties: serde_json::Value) -> String {
    base64::engine::general_purpose::STANDARD.encode(properties.to_string())
}

/// Template the web client uses for "Create My Own".
const CREATE_GUILD_TEMPLATE: &str = "2TffvPucqHkN";
/// Major parameter for routes that are not scoped to a channel or guild.
const NO_MAJOR: Snowflake = Snowflake(0);

#[derive(Default)]
struct Bucket {
    remaining: Option<u64>,
    reset_at: Option<Instant>,
}

#[derive(Default)]
struct State {
    route_hash: HashMap<&'static str, String>,
    buckets: HashMap<String, Arc<tokio::sync::Mutex<Bucket>>>,
    global_until: Option<Instant>,
    window_start: Option<Instant>,
    window_count: u32,
    invalid_start: Option<Instant>,
    invalid_count: u32,
}

#[derive(Deserialize)]
struct RateLimited {
    retry_after: f64,
}

pub struct Rest {
    http: reqwest::Client,
    /// Plain client for cdn.discordapp.com: no API headers, no auth.
    cdn: reqwest::Client,
    identity: Arc<Identity>,
    api: String,
    state: Mutex<State>,
}

impl Rest {
    pub fn new(identity: Arc<Identity>) -> Result<Self> {
        // Same header set the web client sends on its API calls.
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("*/*"));
        headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_str(&identity.accept_language)?);
        headers.insert(REFERER, HeaderValue::from_static("https://discord.com/channels/@me"));
        headers.insert("sec-ch-ua", HeaderValue::from_static(identity::SEC_CH_UA));
        headers.insert("sec-ch-ua-mobile", HeaderValue::from_static("?0"));
        headers.insert("sec-ch-ua-platform", HeaderValue::from_static("\"macOS\""));
        headers.insert("sec-fetch-dest", HeaderValue::from_static("empty"));
        headers.insert("sec-fetch-mode", HeaderValue::from_static("cors"));
        headers.insert("sec-fetch-site", HeaderValue::from_static("same-origin"));
        headers.insert("x-debug-options", HeaderValue::from_static("bugReporterEnabled"));
        headers.insert("x-discord-locale", HeaderValue::from_str(&identity.locale)?);
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
        Ok(Self {
            http,
            cdn,
            api: format!("https://discord.com/api/v{}", identity::API_VERSION),
            identity,
            state: Mutex::default(),
        })
    }

    /// Downloads an image from the CDN, refusing anything implausibly large
    /// for an avatar.
    pub async fn cdn_image(&self, url: &str) -> Result<Vec<u8>> {
        const MAX_BYTES: usize = 2 * 1024 * 1024;
        let response = self.cdn.get(url).send().await?.error_for_status()?;
        if response.content_length().is_some_and(|len| len as usize > MAX_BYTES) {
            bail!("imagem grande demais");
        }
        let bytes = response.bytes().await?;
        if bytes.len() > MAX_BYTES {
            bail!("imagem grande demais");
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
        let response = self
            .request(Method::GET, "GET /channels/{id}/messages", channel, &path, None, None)
            .await?;
        let mut messages: Vec<Message> = response.json().await?;
        messages.reverse();
        Ok(messages)
    }

    pub async fn send_message(&self, channel: Snowflake, content: &str) -> Result<()> {
        let body = json!({ "content": content, "nonce": nonce().to_string(), "tts": false });
        self.request(
            Method::POST,
            "POST /channels/{id}/messages",
            channel,
            &format!("/channels/{channel}/messages"),
            Some(&body),
            None,
        )
        .await?;
        Ok(())
    }

    /// Revokes the token server-side, as the web client does on "Log Out".
    pub async fn logout(&self) -> Result<()> {
        let body = json!({ "provider": null, "voip_provider": null });
        self.request(Method::POST, "POST /auth/logout", NO_MAJOR, "/auth/logout", Some(&body), None)
            .await?;
        Ok(())
    }

    pub async fn send_friend_request(&self, username: &str, captcha: Option<&Solution>) -> Result<()> {
        let body = json!({ "username": username, "discriminator": null });
        self.request_with(
            Method::POST,
            "POST /users/@me/relationships",
            NO_MAJOR,
            "/users/@me/relationships",
            Some(&body),
            Some(&context(json!({ "location": "Add Friend" }))),
            captcha,
        )
        .await?;
        Ok(())
    }

    pub async fn accept_friend(&self, user: Snowflake) -> Result<()> {
        self.request(
            Method::PUT,
            "PUT /users/@me/relationships/{id}",
            NO_MAJOR,
            &format!("/users/@me/relationships/{user}"),
            Some(&json!({})),
            Some(&context(json!({ "location": "Friends" }))),
        )
        .await?;
        Ok(())
    }

    /// Removes a friend, declines an incoming request or cancels an outgoing one.
    pub async fn remove_relationship(&self, user: Snowflake) -> Result<()> {
        self.request(
            Method::DELETE,
            "DELETE /users/@me/relationships/{id}",
            NO_MAJOR,
            &format!("/users/@me/relationships/{user}"),
            None,
            Some(&context(json!({ "location": "Friends" }))),
        )
        .await?;
        Ok(())
    }

    pub async fn open_dm(&self, user: Snowflake, captcha: Option<&Solution>) -> Result<Channel> {
        let body = json!({ "recipients": [user.to_string()] });
        let response = self
            .request_with(
                Method::POST,
                "POST /users/@me/channels",
                NO_MAJOR,
                "/users/@me/channels",
                Some(&body),
                Some(&context(json!({}))),
                captcha,
            )
            .await?;
        Ok(response.json().await?)
    }

    /// Resolves the invite first, like the web client, so the join carries
    /// the guild and channel it came from.
    pub async fn join_guild(&self, code: &str, captcha: Option<&Solution>) -> Result<()> {
        let invite: Invite = self
            .request(
                Method::GET,
                "GET /invites/{code}",
                NO_MAJOR,
                &format!("/invites/{code}?with_counts=true&with_expiration=true"),
                None,
                None,
            )
            .await?
            .json()
            .await?;
        let properties = json!({
            "location": "Join Guild",
            "location_guild_id": invite.guild.as_ref().map(|g| g.id.to_string()),
            "location_channel_id": invite.channel.as_ref().map(|c| c.id.to_string()),
            "location_channel_type": invite.channel.as_ref().map(|c| c.kind),
        });
        let body = json!({ "session_id": self.identity.session_id() });
        self.request_with(
            Method::POST,
            "POST /invites/{code}",
            NO_MAJOR,
            &format!("/invites/{code}"),
            Some(&body),
            Some(&context(properties)),
            captcha,
        )
        .await?;
        Ok(())
    }

    pub async fn create_guild(&self, name: &str, captcha: Option<&Solution>) -> Result<()> {
        let body = json!({
            "name": name,
            "icon": null,
            "channels": [],
            "system_channel_id": null,
            "guild_template_code": CREATE_GUILD_TEMPLATE,
        });
        self.request_with(
            Method::POST,
            "POST /guilds",
            NO_MAJOR,
            "/guilds",
            Some(&body),
            None,
            captcha,
        )
        .await?;
        Ok(())
    }

    /// `None` clears the display name back to the username.
    pub async fn set_global_name(&self, name: Option<&str>) -> Result<User> {
        let response = self
            .request(
                Method::PATCH,
                "PATCH /users/@me",
                NO_MAJOR,
                "/users/@me",
                Some(&json!({ "global_name": name })),
                None,
            )
            .await?;
        Ok(response.json().await?)
    }

    async fn request(
        &self,
        method: Method,
        route: &'static str,
        major: Snowflake,
        path: &str,
        body: Option<&serde_json::Value>,
        context: Option<&str>,
    ) -> Result<Response> {
        self.request_with(method, route, major, path, body, context, None)
            .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn request_with(
        &self,
        method: Method,
        route: &'static str,
        major: Snowflake,
        path: &str,
        body: Option<&serde_json::Value>,
        context: Option<&str>,
        captcha: Option<&Solution>,
    ) -> Result<Response> {
        for _ in 0..MAX_ATTEMPTS {
            let bucket = self.bucket(route, major);
            let mut bucket = bucket.lock().await;
            if bucket.remaining == Some(0) {
                if let Some(reset_at) = bucket.reset_at {
                    sleep_until(reset_at).await;
                }
                bucket.remaining = None;
            }
            self.global_gate().await?;

            let mut request = self
                .http
                .request(method.clone(), format!("{}{path}", self.api))
                .header(AUTHORIZATION, &*self.identity.token());
            if let Some(body) = body {
                request = request.header(ORIGIN, "https://discord.com").json(body);
            }
            if let Some(context) = context {
                request = request.header("x-context-properties", context);
            }
            if let Some(captcha) = captcha {
                request = request.header("x-captcha-key", &captcha.key);
                if let Some(rqtoken) = &captcha.rqtoken {
                    request = request.header("x-captcha-rqtoken", rqtoken);
                }
                if let Some(session_id) = &captcha.session_id {
                    request = request.header("x-captcha-session-id", session_id);
                }
            }
            let response = request.send().await?;
            self.observe_headers(route, &mut bucket, response.headers());

            match response.status() {
                StatusCode::TOO_MANY_REQUESTS => {
                    let global = response.headers().contains_key("x-ratelimit-global");
                    let retry_after = response
                        .json::<RateLimited>()
                        .await
                        .map_or(1.0, |r| r.retry_after);
                    let until = Instant::now() + Duration::from_secs_f64(retry_after.max(0.0));
                    log::warn!("429 on {route}, retry in {retry_after}s (global: {global})");
                    let mut state = self.state.lock().unwrap();
                    Self::note_invalid(&mut state);
                    if global {
                        state.global_until = Some(until);
                    } else {
                        bucket.remaining = Some(0);
                        bucket.reset_at = Some(until);
                    }
                }
                StatusCode::UNAUTHORIZED => {
                    Self::note_invalid(&mut self.state.lock().unwrap());
                    bail!(Unauthorized);
                }
                StatusCode::FORBIDDEN => {
                    Self::note_invalid(&mut self.state.lock().unwrap());
                    bail!(Forbidden(ErrorBody::read(response).await.message));
                }
                status if status.is_success() => return Ok(response),
                status => {
                    let error = ErrorBody::read(response).await;
                    if let (Some(_), Some(sitekey)) = (&error.captcha_key, error.captcha_sitekey) {
                        bail!(CaptchaRequired(Challenge {
                            sitekey,
                            rqdata: error.captcha_rqdata,
                            rqtoken: error.captcha_rqtoken,
                            session_id: error.captcha_session_id,
                        }));
                    }
                    if error.captcha_key.is_some() {
                        bail!("O Discord recusou a verificação. Tente de novo.");
                    }
                    match error.message {
                        Some(message) => bail!("{message}"),
                        None => bail!("HTTP {status}"),
                    }
                }
            }
        }
        bail!("rate limit persistente em {route}")
    }

    fn bucket(&self, route: &'static str, major: Snowflake) -> Arc<tokio::sync::Mutex<Bucket>> {
        let mut state = self.state.lock().unwrap();
        let key = match state.route_hash.get(route) {
            Some(hash) => format!("{hash}:{major}"),
            None => format!("{route}:{major}"),
        };
        if state.buckets.len() > MAX_BUCKETS {
            let now = Instant::now();
            state.buckets.retain(|_, b| {
                Arc::strong_count(b) > 1
                    || b.try_lock()
                        .map_or(true, |b| b.reset_at.is_some_and(|r| r > now))
            });
        }
        state.buckets.entry(key).or_default().clone()
    }

    fn observe_headers(&self, route: &'static str, bucket: &mut Bucket, headers: &HeaderMap) {
        let header = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
        if let Some(remaining) = header("x-ratelimit-remaining").and_then(|v| v.parse().ok()) {
            bucket.remaining = Some(remaining);
        }
        if let Some(after) = header("x-ratelimit-reset-after").and_then(|v| v.parse::<f64>().ok()) {
            bucket.reset_at = Some(Instant::now() + Duration::from_secs_f64(after.max(0.0)));
        }
        if let Some(hash) = header("x-ratelimit-bucket") {
            let mut state = self.state.lock().unwrap();
            if state.route_hash.get(route).map(String::as_str) != Some(hash) {
                state.route_hash.insert(route, hash.to_owned());
            }
        }
    }

    async fn global_gate(&self) -> Result<()> {
        loop {
            let wait = {
                let mut state = self.state.lock().unwrap();
                let now = Instant::now();
                if state.invalid_count >= INVALID_CAP
                    && state.invalid_start.is_some_and(|s| now - s < INVALID_WINDOW)
                {
                    bail!("muitas requisições inválidas; pausado para evitar bloqueio");
                }
                match state.global_until {
                    Some(until) if until > now => Some(until - now),
                    _ => {
                        let start = *state.window_start.get_or_insert(now);
                        if now - start >= Duration::from_secs(1) {
                            state.window_start = Some(now);
                            state.window_count = 0;
                        }
                        if state.window_count < GLOBAL_PER_SECOND {
                            state.window_count += 1;
                            None
                        } else {
                            Some(start + Duration::from_secs(1) - now)
                        }
                    }
                }
            };
            match wait {
                None => return Ok(()),
                Some(wait) => sleep(wait).await,
            }
        }
    }

    fn note_invalid(state: &mut State) {
        let now = Instant::now();
        match state.invalid_start {
            Some(start) if now - start < INVALID_WINDOW => state.invalid_count += 1,
            _ => {
                state.invalid_start = Some(now);
                state.invalid_count = 1;
            }
        }
    }
}

fn nonce() -> u64 {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    ms.saturating_sub(DISCORD_EPOCH_MS) << 22
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

#[cfg(test)]
mod tests {
    use super::invite_code;

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
