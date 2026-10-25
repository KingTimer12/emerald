//! Client identity sent to Discord, mirroring the stable web client as
//! observed with REA on 2026-10-19 (Chrome 155, macOS). On Windows the OS
//! fields switch to what the same Chrome reports there, matching the WebView2
//! that performed the login. Every REST request
//! carries `X-Super-Properties` and the gateway IDENTIFY carries the same
//! object, so both must stay consistent.
//!
//! `launch_signature` is produced by the web client's libdiscore WASM; the
//! web client itself sends `null` before that module initializes, which is
//! what this client sends. `qos_token` is omitted for the same reason.

use std::sync::{Arc, RwLock};

use base64::Engine;
use serde_json::{Value, json};

use super::token::{self, Token};

/// `capabilities` the web client sends when channel obfuscation is off.
pub const CAPABILITIES: u64 = 1_734_653;
#[cfg(not(windows))]
pub const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36";
#[cfg(windows)]
pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36";
/// `sec-ch-ua-platform` header value.
#[cfg(not(windows))]
pub const SEC_CH_UA_PLATFORM: &str = "\"macOS\"";
#[cfg(windows)]
pub const SEC_CH_UA_PLATFORM: &str = "\"Windows\"";
/// `os` and `os_version` super properties.
#[cfg(not(windows))]
const OS: (&str, &str) = ("Mac OS X", "10.15.7");
#[cfg(windows)]
const OS: (&str, &str) = ("Windows", "10");
pub const SEC_CH_UA: &str = r#""Google Chrome";v="155", "Chromium";v="155", "Not(A:Brand";v="24""#;
const BROWSER_VERSION: &str = "155.0.0.0";

pub struct Identity {
    pub properties: Value,
    pub super_properties: String,
    /// `x-discord-locale` and `Accept-Language`, which follow the interface
    /// language so Discord answers in it.
    language: RwLock<(Arc<str>, Arc<str>)>,
    pub timezone: String,
    token: RwLock<Arc<Token>>,
    /// Gateway session id; the web client sends it when accepting invites.
    session_id: RwLock<Option<String>>,
}

impl Identity {
    /// `build_number` comes from `client_build::current`.
    pub fn new(token: Token, build_number: u64) -> Self {
        let locale = locale();

        // Key order matches the captured header; serde_json preserves it.
        let properties = json!({
            "os": OS.0,
            "browser": "Chrome",
            "device": "",
            "device_model_name": "",
            "system_locale": locale,
            "has_client_mods": false,
            "browser_user_agent": USER_AGENT,
            "browser_version": BROWSER_VERSION,
            "os_version": OS.1,
            "referrer": "",
            "referring_domain": "",
            "referrer_current": "",
            "referring_domain_current": "",
            "release_channel": "stable",
            "client_build_number": build_number,
            "client_event_source": null,
            "client_launch_id": uuid::Uuid::new_v4().to_string(),
            "launch_signature": null,
            "client_heartbeat_session_id": uuid::Uuid::new_v4().to_string(),
            "client_app_state": "focused",
        });
        let super_properties =
            base64::engine::general_purpose::STANDARD.encode(properties.to_string());

        Self {
            properties,
            super_properties,
            language: RwLock::new((locale.as_str().into(), accept_language(&locale).into())),
            timezone: system_timezone(),
            token: RwLock::new(Arc::new(token)),
            session_id: RwLock::new(None),
        }
    }

    pub fn locale(&self) -> Arc<str> {
        self.language.read().unwrap().0.clone()
    }

    pub fn accept_language(&self) -> Arc<str> {
        self.language.read().unwrap().1.clone()
    }

    /// The interface switched languages. `EMERALD_LOCALE` still wins.
    pub fn set_locale(&self, locale: &str) {
        if std::env::var_os("EMERALD_LOCALE").is_none() {
            *self.language.write().unwrap() = (locale.into(), accept_language(locale).into());
        }
    }

    pub fn token(&self) -> Arc<Token> {
        self.token.read().unwrap().clone()
    }

    /// The account's user id: a token's first segment is it in base64.
    /// Zero for a token in another shape.
    pub fn account(&self) -> u64 {
        let token = self.token();
        let first = token.expose().split('.').next().unwrap_or_default().trim_end_matches('=');
        base64::engine::general_purpose::STANDARD_NO_PAD
            .decode(first)
            .ok()
            .and_then(|id| String::from_utf8(id).ok()?.parse().ok())
            .unwrap_or(0)
    }

    pub fn session_id(&self) -> Option<String> {
        self.session_id.read().unwrap().clone()
    }

    pub fn set_session_id(&self, id: String) {
        *self.session_id.write().unwrap() = Some(id);
    }

    /// READY may carry a rotated token (AUTH_TOKEN_REFRESH capability);
    /// the old one must not be reused afterwards.
    pub fn replace_token(&self, token: String) {
        let Some(token) = Token::new(token) else { return };
        if let Err(e) = token::save(&token) {
            tracing::error!("could not persist rotated token: {e}");
        }
        *self.token.write().unwrap() = Arc::new(token);
    }
}

/// Discord answers errors in this language, so it follows the interface.
pub fn locale() -> String {
    std::env::var("EMERALD_LOCALE").unwrap_or_else(|_| emerald_core::i18n::current().to_owned())
}

pub fn accept_language(locale: &str) -> String {
    match locale.split_once('-') {
        Some((lang, _)) if lang != "en" => {
            format!("{locale},{lang};q=0.9,en-US;q=0.8,en;q=0.7")
        }
        _ => "en-US,en;q=0.9".into(),
    }
}

/// IANA name of the system timezone, as the browser reports it.
fn system_timezone() -> String {
    std::env::var("TZ")
        .ok()
        .or_else(platform_timezone)
        .unwrap_or_else(|| "UTC".into())
}

#[cfg(not(windows))]
fn platform_timezone() -> Option<String> {
    let target = std::fs::read_link("/etc/localtime").ok()?;
    let path = target.to_str()?;
    Some(path.split_once("zoneinfo/")?.1.to_owned())
}

/// Windows' own zone ids ("E. South America Standard Time") are not IANA;
/// the WinRT calendar maps them the same way Edge does.
#[cfg(windows)]
fn platform_timezone() -> Option<String> {
    let calendar = windows::Globalization::Calendar::new().ok()?;
    Some(calendar.GetTimeZone().ok()?.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Key order of the `X-Super-Properties` header captured from the web client.
    const CAPTURED_KEYS: [&str; 20] = [
        "os", "browser", "device", "device_model_name", "system_locale", "has_client_mods",
        "browser_user_agent", "browser_version", "os_version", "referrer", "referring_domain",
        "referrer_current", "referring_domain_current", "release_channel", "client_build_number",
        "client_event_source", "client_launch_id", "launch_signature",
        "client_heartbeat_session_id", "client_app_state",
    ];

    #[test]
    fn super_properties_match_web_client_shape() {
        let identity = Identity::new(Token::new("t".into()).unwrap(), 1);
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&identity.super_properties)
            .unwrap();
        let value: Value = serde_json::from_slice(&decoded).unwrap();
        let keys: Vec<&str> = value.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, CAPTURED_KEYS);
        assert_eq!(value["browser_user_agent"], USER_AGENT);
    }

    #[test]
    fn account_comes_from_the_token() {
        // base64("1547312596632338476"), no padding, as Discord issues it.
        let token = Token::new("MTU0NzMxMjU5NjYzMjMzODQ3Ng.x.y".into()).unwrap();
        assert_eq!(Identity::new(token, 1).account(), 1_547_312_596_632_338_476);
        assert_eq!(Identity::new(Token::new("junk".into()).unwrap(), 1).account(), 0);
    }

    #[test]
    fn accept_language_matches_capture() {
        assert_eq!(accept_language("pt-BR"), "pt-BR,pt;q=0.9,en-US;q=0.8,en;q=0.7");
    }
}
