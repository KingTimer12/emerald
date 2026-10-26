//! Requests to one API under a [`BaseUrl`]: rate limits, the microcache,
//! stale copies on error, and typed failures.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, bail};
use bytes::Bytes;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Method, Response, StatusCode};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use tokio::time::Instant;

use crate::base_url::BaseUrl;
use crate::cache::Cache;
use crate::downloads::on_disk;
use crate::error::{ErrorBody, Forbidden, StillLimited, Unavailable};
use crate::microcache::{Lookup, MicroCache, Policy, Ticket};
use crate::ratelimit::{RateLimiter, Route};
use crate::{NO_SCOPE, Scope};

const MAX_ATTEMPTS: u32 = 4;

#[derive(Deserialize)]
struct RateLimited {
    retry_after: f64,
}

/// Who the requests are made as: headers that can change during a run
/// (credentials, language), and whose answers the disk keeps.
pub trait Session: Send + Sync {
    /// Adds this moment's headers to a request.
    fn headers(&self, headers: &mut HeaderMap) -> Result<()>;
    /// The account the disk cache files kept answers under.
    fn account(&self) -> u64;
}

/// One API request, built up step by step. The route template names its
/// rate-limit bucket and `major` (channel or guild) scopes it.
pub struct Call {
    route: Route,
    major: Scope,
    path: String,
    body: Option<serde_json::Value>,
    headers: Vec<(HeaderName, HeaderValue)>,
    cache: Option<Policy>,
}

impl Call {
    /// `path` is joined to the client's base URL.
    pub fn new(method: Method, route: &'static str, path: impl Into<String>) -> Self {
        Self {
            route: (method, route),
            major: NO_SCOPE,
            path: path.into(),
            body: None,
            headers: Vec::new(),
            cache: None,
        }
    }

    pub fn get(route: &'static str, path: impl Into<String>) -> Self {
        Self::new(Method::GET, route, path)
    }

    pub fn post(route: &'static str, path: impl Into<String>) -> Self {
        Self::new(Method::POST, route, path)
    }

    pub fn put(route: &'static str, path: impl Into<String>) -> Self {
        Self::new(Method::PUT, route, path)
    }

    pub fn patch(route: &'static str, path: impl Into<String>) -> Self {
        Self::new(Method::PATCH, route, path)
    }

    pub fn delete(route: &'static str, path: impl Into<String>) -> Self {
        Self::new(Method::DELETE, route, path)
    }

    /// Scopes the bucket to a channel or guild, as Discord does.
    pub fn major(mut self, major: impl Into<Scope>) -> Self {
        self.major = major.into();
        self
    }

    /// Serves a GET from the microcache under `policy`.
    pub fn cached(mut self, policy: Policy) -> Self {
        debug_assert_eq!(self.route.0, Method::GET);
        self.cache = Some(policy);
        self
    }

    pub fn json(mut self, body: serde_json::Value) -> Self {
        self.body = Some(body);
        self
    }

    /// One more header on this request only. Values that are not valid
    /// header text are dropped.
    pub fn header(mut self, name: &'static str, value: &str) -> Self {
        if let Ok(value) = HeaderValue::from_str(value) {
            self.headers.push((HeaderName::from_static(name), value));
        }
        self
    }
}

pub struct Client {
    http: reqwest::Client,
    base: BaseUrl,
    session: Arc<dyn Session>,
    /// Sent only with requests that carry a body, as browsers send
    /// `Origin` on writes.
    body_headers: HeaderMap,
    /// Kept responses on disk; `None` runs uncached.
    cache: Option<Arc<Cache>>,
    micro: MicroCache,
    limits: RateLimiter,
}

impl Client {
    /// `http` carries the headers every request shares; `session` adds the
    /// ones that change.
    pub fn new(http: reqwest::Client, base: BaseUrl, session: Arc<dyn Session>, cache: Option<Arc<Cache>>) -> Self {
        Self {
            http,
            base,
            session,
            body_headers: HeaderMap::new(),
            cache,
            micro: MicroCache::default(),
            limits: RateLimiter::default(),
        }
    }

    /// Adds a header sent with every request that has a body.
    pub fn with_body_header(mut self, name: HeaderName, value: HeaderValue) -> Self {
        self.body_headers.insert(name, value);
        self
    }

    /// Sends `call` and decodes the JSON answer.
    pub async fn fetch<T: DeserializeOwned>(&self, call: Call) -> Result<T> {
        match call.cache {
            Some(policy) => Ok(serde_json::from_slice(&self.cached(&call, policy).await?)?),
            None => Ok(self.send(&call).await?.json().await?),
        }
    }

    /// Sends `call` for its effect, ignoring the body.
    pub async fn run(&self, call: Call) -> Result<()> {
        self.send(&call).await.map(drop)
    }

    /// The answer to a GET, from the microcache when it has a usable copy.
    async fn cached(&self, call: &Call, policy: Policy) -> Result<Bytes> {
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

    async fn refresh(&self, call: &Call, policy: Policy, ticket: Ticket<'_>) -> Result<Bytes> {
        let result = async { Ok::<_, anyhow::Error>(self.send(call).await?.bytes().await?) }.await;
        let error = match result {
            Ok(body) => {
                if policy.persist {
                    let (account, kept) = (self.session.account(), body.clone());
                    on_disk(&self.cache, move |cache, key| cache.put_response(account, key, &kept), ticket.key())
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
            let account = self.session.account();
            let kept = on_disk(&self.cache, move |cache, key| cache.response(account, key), ticket.key()).await;
            stand_in = kept.flatten().map(|(body, _)| body.into());
        }
        match stand_in {
            Some(body) => {
                tracing::info!("{} failed ({error}); serving a cached copy", call.route.1);
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
    pub async fn send(&self, call: &Call) -> Result<Response> {
        let route = &call.route;
        for _ in 0..MAX_ATTEMPTS {
            let mut bucket = self.limits.acquire(route, call.major).await?;

            let mut headers = HeaderMap::new();
            self.session.headers(&mut headers)?;
            if call.body.is_some() {
                headers.extend(self.body_headers.clone());
            }
            headers.extend(call.headers.iter().map(|(name, value)| (Some(name.clone()), value.clone())));
            let mut request = self.http.request(route.0.clone(), self.base.join(&call.path)).headers(headers);
            if let Some(body) = &call.body {
                request = request.json(body);
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
                    tracing::warn!("429 on {} {}, retry in {retry_after}s (global: {global})", route.0, route.1);
                    self.limits.note_invalid();
                    if global {
                        self.limits.block_global(until);
                    } else {
                        bucket.block(until);
                    }
                }
                StatusCode::UNAUTHORIZED => {
                    self.limits.note_invalid();
                    return Err(ErrorBody::read(response).await.unauthorized());
                }
                StatusCode::FORBIDDEN => {
                    self.limits.note_invalid();
                    bail!(Forbidden(ErrorBody::read(response).await.message()));
                }
                status if status.is_success() => {
                    // A write makes what was read from the same channel or guild stale.
                    if route.0 != Method::GET && call.major != NO_SCOPE {
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
        bail!(StillLimited(format!("{} {}", route.0, route.1)))
    }
}

/// Failures worth answering from a stale copy: the server or the network
/// could not answer, as opposed to answering no.
fn is_transient(error: &anyhow::Error) -> bool {
    error.is::<Unavailable>() || error.is::<StillLimited>() || error.is::<reqwest::Error>()
}
