//! Typed REST failures the network layer reacts to, and the captcha
//! round trip.

use reqwest::Response;
use serde::{Deserialize, Serialize};

/// 403, with Discord's (localized) reason when the body has one.
#[derive(Debug)]
pub struct Forbidden(pub Option<String>);

impl std::fmt::Display for Forbidden {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_deref().unwrap_or(t!("errors.forbidden")))
    }
}

impl std::error::Error for Forbidden {}

/// 401: the token was revoked or expired; the session must sign in again.
#[derive(Debug)]
pub struct Unauthorized;

impl std::fmt::Display for Unauthorized {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(t!("errors.unauthorized"))
    }
}

impl std::error::Error for Unauthorized {}

/// Discord could not answer: a 5xx, or still rate limited after every retry.
#[derive(Debug)]
pub struct Unavailable(pub String);

impl std::fmt::Display for Unavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Unavailable {}

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
        f.write_str(t!("errors.captcha_incomplete"))
    }
}

impl std::error::Error for CaptchaRequired {}

/// Error body Discord returns on 4xx. `message` is localized through
/// `X-Discord-Locale`, so it is shown to the user as is.
#[derive(Default, Deserialize)]
pub(super) struct ErrorBody {
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
    pub(super) async fn read(response: Response) -> Self {
        response.json().await.unwrap_or_default()
    }

    pub(super) fn message(self) -> Option<String> {
        self.message
    }

    /// The error a failed (non-401/403/429) response stands for.
    pub(super) fn into_error(self, status: reqwest::StatusCode) -> anyhow::Error {
        if let (Some(_), Some(sitekey)) = (&self.captcha_key, self.captcha_sitekey) {
            return CaptchaRequired(Challenge {
                sitekey,
                rqdata: self.captcha_rqdata,
                rqtoken: self.captcha_rqtoken,
                session_id: self.captcha_session_id,
            })
            .into();
        }
        if self.captcha_key.is_some() {
            return anyhow::anyhow!(t!("errors.captcha_rejected"));
        }
        match self.message {
            Some(message) => anyhow::anyhow!("{message}"),
            None => anyhow::anyhow!("HTTP {status}"),
        }
    }
}
