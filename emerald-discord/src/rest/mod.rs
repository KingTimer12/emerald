//! Discord's REST endpoints, each as the web client calls it. Requests go
//! through [`emerald_http::Client`] under [`urls::API`], which applies rate
//! limits, the microcache and typed errors; bodies come from `packets`.
//! The endpoints are grouped by area, one module each.

mod apps;
mod encoding;
mod expressions;
mod guilds;
mod ids;
mod messages;
mod threads;
mod users;

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use base64::Engine;
use emerald_http::{Call, Client, Downloads, Policy, Session};
use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, AUTHORIZATION, HeaderMap, HeaderValue, ORIGIN, REFERER, USER_AGENT};

pub use crate::packets::message::Outgoing;
pub use crate::packets::thread::ThreadSearch;
pub use crate::packets::upload::Uploaded;
pub use emerald_http::{CaptchaRequired, Challenge, Forbidden, Solution, Unauthorized, describe};
pub use ids::{invite_code, nonce};

use super::identity::{self, Identity};
use super::urls;

pub const HISTORY_PAGE: usize = 50;
/// Largest still image downloaded.
const MAX_IMAGE_BYTES: usize = 2 * 1024 * 1024;
/// Largest animated GIF or WebP downloaded; they are scaled down as they
/// are decoded, so this bounds the download, not the memory.
const MAX_ANIMATION_BYTES: usize = 12 * 1024 * 1024;

/// Profiles: the microcache's second, a stale copy for ten minutes while a
/// refresh is slow or failing, and the last one on disk after a restart.
const PROFILE_CACHE: Policy = Policy {
    fresh: Duration::from_secs(1),
    stale: Duration::from_secs(600),
    persist: true,
};

/// What Discord adds to a [`Call`] beyond plain HTTP.
trait DiscordCall {
    /// `X-Context-Properties`: base64 JSON telling Discord which UI surface
    /// triggered the request, as the web client sends on relationship and
    /// invite calls.
    fn context(self, properties: serde_json::Value) -> Self;
    /// The `X-Captcha-*` headers of a solved challenge, on the retry.
    fn captcha(self, captcha: Option<&Solution>) -> Self;
}

impl DiscordCall for Call {
    fn context(self, properties: serde_json::Value) -> Self {
        let encoded = base64::engine::general_purpose::STANDARD.encode(properties.to_string());
        self.header("x-context-properties", &encoded)
    }

    fn captcha(self, captcha: Option<&Solution>) -> Self {
        let Some(captcha) = captcha else { return self };
        let mut call = self.header("x-captcha-key", &captcha.key);
        if let Some(rqtoken) = &captcha.rqtoken {
            call = call.header("x-captcha-rqtoken", rqtoken);
        }
        if let Some(session_id) = &captcha.session_id {
            call = call.header("x-captcha-session-id", session_id);
        }
        call
    }
}

/// The account behind each request: its token, and the language Discord
/// should answer in, which follows the interface.
impl Session for Identity {
    fn headers(&self, headers: &mut HeaderMap) -> Result<()> {
        // Sensitive: kept out of debug output and never HPACK-indexed.
        let mut authorization = HeaderValue::from_str(self.token().expose())?;
        authorization.set_sensitive(true);
        headers.insert(AUTHORIZATION, authorization);
        headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_str(&self.accept_language())?);
        headers.insert("x-discord-locale", HeaderValue::from_str(&self.locale())?);
        Ok(())
    }

    fn account(&self) -> u64 {
        Identity::account(self)
    }
}

pub struct Rest {
    api: Client,
    /// CDN images and media, through the disk cache.
    cdn: Downloads,
    /// Puts attachments to the storage URLs Discord hands out; never sends
    /// the token, as the browser's cross-origin upload does not.
    uploads: reqwest::Client,
    identity: Arc<Identity>,
}

impl Rest {
    pub fn new(identity: Arc<Identity>, cache: Option<Arc<emerald_http::Cache>>) -> Result<Self> {
        // Same header set the web client sends on its API calls.
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("*/*"));
        headers.insert(REFERER, HeaderValue::from_str(&urls::WEB.join("/channels/@me"))?);
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
        let api = Client::new(http, urls::API, identity.clone(), cache.clone())
            .with_body_header(ORIGIN, HeaderValue::from_static(urls::WEB.as_str()));
        let mut upload_headers = HeaderMap::new();
        upload_headers.insert(ORIGIN, HeaderValue::from_static(urls::WEB.as_str()));
        upload_headers.insert(REFERER, HeaderValue::from_str(&urls::WEB.join("/"))?);
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
            api,
            cdn: Downloads::new(identity::USER_AGENT, cache)?,
            uploads,
            identity,
        })
    }

    /// Fetches a CDN image from the disk cache, or downloads and caches it.
    /// Refuses anything implausibly large for an avatar or sticker.
    pub async fn cdn_image(&self, url: &str) -> Result<Vec<u8>> {
        self.cdn.get(url, MAX_IMAGE_BYTES).await
    }

    /// An animated GIF or WebP, which may be larger than other images.
    pub async fn cdn_animation(&self, url: &str) -> Result<Vec<u8>> {
        self.cdn.get(url, MAX_ANIMATION_BYTES).await
    }
}

/// Archived forum posts per page, as the web client loads them.
pub const FORUM_PAGE: usize = 25;
