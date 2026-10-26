//! HTTP transport for Emerald. A [`Client`] speaks to one [`BaseUrl`], so a
//! request names only its path; every call goes through the
//! [`RateLimiter`](ratelimit), GETs that opt in go through the
//! [`microcache`], and errors come back as the typed failures in
//! [`error`]. [`Downloads`] fetches whole URLs (CDN images and media)
//! through the disk [`Cache`].

#[macro_use]
extern crate emerald_core;

mod base_url;
mod cache;
mod client;
mod downloads;
mod error;
mod microcache;
mod ratelimit;

pub use base_url::BaseUrl;
pub use cache::Cache;
pub use client::{Call, Client, Session};
pub use downloads::Downloads;
pub use error::{CaptchaRequired, Challenge, Forbidden, Reauth, Refused, Solution, Unauthorized, describe};
pub use microcache::Policy;

/// What a request is scoped to for rate limits and the microcache: the
/// channel or guild id in its path (Discord's "major parameter"), or 0.
pub type Scope = u64;

/// Requests not scoped to a channel or guild.
pub const NO_SCOPE: Scope = 0;
