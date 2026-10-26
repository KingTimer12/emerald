//! Typed REST failures the network layer reacts to, and the captcha
//! round trip. Their `Display` is English, for logs; [`describe`] words
//! them for the user, in the interface language.

use reqwest::Response;
use serde::{Deserialize, Serialize};

/// 403, with Discord's (localized) reason when the body has one.
#[derive(Debug)]
pub struct Forbidden(pub Option<String>);

impl std::fmt::Display for Forbidden {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("forbidden (403)")
    }
}

impl std::error::Error for Forbidden {}

/// 401: the token was revoked or expired; the session must sign in again.
#[derive(Debug)]
pub struct Unauthorized;

impl std::fmt::Display for Unauthorized {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("unauthorized (401)")
    }
}

impl std::error::Error for Unauthorized {}

/// 401 asking the user to confirm it is them (password or two-factor code)
/// before a sensitive change. The token is still good; Emerald has no such
/// prompt, so the change has to be made in Discord's own app.
#[derive(Debug)]
pub struct Reauth;

impl std::fmt::Display for Reauth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("confirmation (MFA) required (401)")
    }
}

impl std::error::Error for Reauth {}

/// Discord's `MFA_REQUIRED` error code.
const MFA_REQUIRED: u64 = 60003;

/// Discord could not answer: a 5xx, or still rate limited after every retry.
#[derive(Debug)]
pub struct Unavailable(pub String);

impl std::fmt::Display for Unavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Unavailable {}

/// Still rate limited on `route` after every retry.
#[derive(Debug)]
pub struct StillLimited(pub String);

impl std::fmt::Display for StillLimited {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "still rate limited on {} after retrying", self.0)
    }
}

impl std::error::Error for StillLimited {}

/// A 4xx Discord explained; `message` is in the user's language, as Discord
/// localizes it through `X-Discord-Locale`.
#[derive(Debug)]
pub struct Refused {
    pub status: reqwest::StatusCode,
    pub message: Option<String>,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP {}", self.status)
    }
}

impl std::error::Error for Refused {}

/// Failures the client words itself.
#[derive(Debug)]
pub enum Local {
    CaptchaRejected,
    ImageTooLarge,
    /// Too many failed requests lately: Discord would ban the IP for a while.
    InvalidRequests,
}

impl std::fmt::Display for Local {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::CaptchaRejected => "the captcha answer was rejected",
            Self::ImageTooLarge => "image too large",
            Self::InvalidRequests => "too many failed requests; pausing to avoid a temporary ban",
        })
    }
}

impl std::error::Error for Local {}

/// `error` worded for the user: typed failures in the interface language,
/// Discord's own (already localized) reasons as given, and anything else
/// as it is.
pub fn describe(error: &anyhow::Error) -> String {
    if let Some(Forbidden(message)) = error.downcast_ref() {
        return message.clone().unwrap_or_else(|| t!("errors.forbidden").to_owned());
    }
    if error.is::<Unauthorized>() {
        return t!("errors.unauthorized").to_owned();
    }
    if error.is::<Reauth>() {
        return t!("errors.reauth").to_owned();
    }
    if error.is::<CaptchaRequired>() {
        return t!("errors.captcha_incomplete").to_owned();
    }
    if let Some(StillLimited(route)) = error.downcast_ref() {
        return t!("errors.rate_limited", route = route);
    }
    if let Some(refused) = error.downcast_ref::<Refused>() {
        return refused.message.clone().unwrap_or_else(|| refused.to_string());
    }
    match error.downcast_ref::<Local>() {
        Some(Local::CaptchaRejected) => t!("errors.captcha_rejected").to_owned(),
        Some(Local::ImageTooLarge) => t!("errors.image_too_large").to_owned(),
        Some(Local::InvalidRequests) => t!("errors.invalid_requests").to_owned(),
        None => error.to_string(),
    }
}

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
        f.write_str("captcha required")
    }
}

impl std::error::Error for CaptchaRequired {}

/// Error body Discord returns on 4xx. `message` is localized through
/// `X-Discord-Locale`, so it is shown to the user as is.
#[derive(Default, Deserialize)]
pub(crate) struct ErrorBody {
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    code: Option<u64>,
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
    pub(crate) async fn read(response: Response) -> Self {
        response.json().await.unwrap_or_default()
    }

    pub(crate) fn message(self) -> Option<String> {
        self.message
    }

    /// What a 401 with this body stands for: a request to confirm the
    /// user's identity, or a token Discord no longer takes.
    pub(crate) fn unauthorized(self) -> anyhow::Error {
        if self.code == Some(MFA_REQUIRED) { Reauth.into() } else { Unauthorized.into() }
    }

    /// The error a failed (non-401/403/429) response stands for.
    pub(crate) fn into_error(self, status: reqwest::StatusCode) -> anyhow::Error {
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
            return Local::CaptchaRejected.into();
        }
        Refused {
            status,
            message: self.message,
        }
        .into()
    }
}
