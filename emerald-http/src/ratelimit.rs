//! Discord's rate-limit contract:
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
use reqwest::Method;
use reqwest::header::HeaderMap;
use tokio::sync::OwnedMutexGuard;
use tokio::time::{Instant, sleep, sleep_until};

use crate::Scope;

const GLOBAL_PER_SECOND: u32 = 25;
const INVALID_WINDOW: Duration = Duration::from_secs(600);
const INVALID_CAP: u32 = 500;
const MAX_BUCKETS: usize = 256;

/// A route as Discord buckets it: method plus path template.
pub type Route = (Method, &'static str);

#[derive(Default)]
pub struct Bucket {
    remaining: Option<u64>,
    reset_at: Option<Instant>,
}

impl Bucket {
    /// Waits out an exhausted bucket.
    async fn ready(&mut self) {
        if self.remaining == Some(0) {
            if let Some(reset_at) = self.reset_at {
                sleep_until(reset_at).await;
            }
            self.remaining = None;
        }
    }

    /// Blocks the bucket until `until`, after a 429 on it.
    pub fn block(&mut self, until: Instant) {
        self.remaining = Some(0);
        self.reset_at = Some(until);
    }
}

#[derive(Default)]
struct State {
    route_hash: HashMap<Route, String>,
    buckets: HashMap<String, Arc<tokio::sync::Mutex<Bucket>>>,
    global_until: Option<Instant>,
    window_start: Option<Instant>,
    window_count: u32,
    invalid_start: Option<Instant>,
    invalid_count: u32,
}

#[derive(Default)]
pub struct RateLimiter {
    state: Mutex<State>,
}

impl RateLimiter {
    /// Holds the route's bucket and a global slot; the guard keeps other
    /// requests to the bucket waiting until the response is observed.
    pub async fn acquire(&self, route: &Route, major: Scope) -> Result<OwnedMutexGuard<Bucket>> {
        let mut bucket = self.bucket(route, major).lock_owned().await;
        bucket.ready().await;
        self.global_gate().await?;
        Ok(bucket)
    }

    /// Learns the bucket's remaining budget and its shared hash.
    pub fn observe(&self, route: &Route, bucket: &mut Bucket, headers: &HeaderMap) {
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
                state.route_hash.insert(route.clone(), hash.to_owned());
            }
        }
    }

    /// Pauses every request until `until`, after a global 429.
    pub fn block_global(&self, until: Instant) {
        self.state.lock().unwrap().global_until = Some(until);
    }

    /// Counts a 401, 403 or 429 toward the invalid-request guard.
    pub fn note_invalid(&self) {
        let mut state = self.state.lock().unwrap();
        let now = Instant::now();
        match state.invalid_start {
            Some(start) if now - start < INVALID_WINDOW => state.invalid_count += 1,
            _ => {
                state.invalid_start = Some(now);
                state.invalid_count = 1;
            }
        }
    }

    fn bucket(&self, route: &Route, major: Scope) -> Arc<tokio::sync::Mutex<Bucket>> {
        let mut state = self.state.lock().unwrap();
        let key = match state.route_hash.get(route) {
            Some(hash) => format!("{hash}:{major}"),
            None => format!("{} {}:{major}", route.0, route.1),
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

    async fn global_gate(&self) -> Result<()> {
        loop {
            let wait = {
                let mut state = self.state.lock().unwrap();
                let now = Instant::now();
                if state.invalid_count >= INVALID_CAP
                    && state.invalid_start.is_some_and(|s| now - s < INVALID_WINDOW)
                {
                    bail!(super::error::Local::InvalidRequests);
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn global_cap_spreads_bursts_over_seconds() {
        let limiter = RateLimiter::default();
        let route = (Method::GET, "/test");
        let start = Instant::now();
        for major in 0..GLOBAL_PER_SECOND as u64 + 1 {
            drop(limiter.acquire(&route, major).await.unwrap());
        }
        assert!(Instant::now() - start >= Duration::from_secs(1));
    }

    #[tokio::test(start_paused = true)]
    async fn too_many_invalid_requests_stop_all_calls() {
        let limiter = RateLimiter::default();
        for _ in 0..INVALID_CAP {
            limiter.note_invalid();
        }
        assert!(limiter.acquire(&(Method::GET, "/test"), 0).await.is_err());
    }
}
