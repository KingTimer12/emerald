//! Client identity sent to Discord, mirroring the stable web client as
//! observed with REA on 2026-10-19 (Chrome 155, macOS). Every REST request
//! carries `X-Super-Properties` and the gateway IDENTIFY carries the same
//! object, so both must stay consistent.
//!
//! `launch_signature` is produced by the web client's libdiscore WASM; the
//! web client itself sends `null` before that module initializes, which is
//! what this client sends. `qos_token` is omitted for the same reason.

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use base64::Engine;
use serde_json::{Value, json};

/// Observed `client_build_number`; it changes with every web deploy.
/// Override with `EMERALD_BUILD_NUMBER` when it goes stale.
const CLIENT_BUILD_NUMBER: u64 = 641_004;
pub const API_VERSION: u8 = 9;
/// `capabilities` the web client sends when channel obfuscation is off.
pub const CAPABILITIES: u64 = 1_734_653;
pub const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36";
pub const SEC_CH_UA: &str = r#""Google Chrome";v="155", "Chromium";v="155", "Not(A:Brand";v="24""#;
const BROWSER_VERSION: &str = "155.0.0.0";

pub struct Identity {
    pub properties: Value,
    pub super_properties: String,
    pub locale: String,
    pub accept_language: String,
    pub timezone: String,
    token: RwLock<Arc<str>>,
    /// Gateway session id; the web client sends it when accepting invites.
    session_id: RwLock<Option<String>>,
}

impl Identity {
    pub fn new(token: String) -> Self {
        let locale = std::env::var("EMERALD_LOCALE").unwrap_or_else(|_| "pt-BR".into());
        let build_number = std::env::var("EMERALD_BUILD_NUMBER")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(CLIENT_BUILD_NUMBER);

        // Key order matches the captured header; serde_json preserves it.
        let properties = json!({
            "os": "Mac OS X",
            "browser": "Chrome",
            "device": "",
            "device_model_name": "",
            "system_locale": locale,
            "has_client_mods": false,
            "browser_user_agent": USER_AGENT,
            "browser_version": BROWSER_VERSION,
            "os_version": "10.15.7",
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
            accept_language: accept_language(&locale),
            locale,
            timezone: system_timezone(),
            token: RwLock::new(token.into()),
            session_id: RwLock::new(None),
        }
    }

    pub fn token(&self) -> Arc<str> {
        self.token.read().unwrap().clone()
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
        if let Err(e) = persist_token(&token) {
            log::error!("could not persist rotated token: {e}");
        }
        if std::env::var_os("DISCORD_TOKEN").is_some() {
            log::warn!("token rotated; DISCORD_TOKEN is now stale, using ~/.config/emerald/token");
        }
        *self.token.write().unwrap() = token.into();
    }
}

pub fn token_path() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("HOME")?).join(".config/emerald/token"))
}

pub fn persist_token(token: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let path = token_path().ok_or(std::io::ErrorKind::NotFound)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&tmp)?;
    file.write_all(token.as_bytes())?;
    file.sync_all()?;
    std::fs::rename(tmp, path)
}

fn accept_language(locale: &str) -> String {
    match locale.split_once('-') {
        Some((lang, _)) if lang != "en" => {
            format!("{locale},{lang};q=0.9,en-US;q=0.8,en;q=0.7")
        }
        _ => "en-US,en;q=0.9".into(),
    }
}

fn system_timezone() -> String {
    std::env::var("TZ")
        .ok()
        .or_else(|| {
            let target = std::fs::read_link("/etc/localtime").ok()?;
            let path = target.to_str()?;
            Some(path.split_once("zoneinfo/")?.1.to_owned())
        })
        .unwrap_or_else(|| "UTC".into())
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
        let identity = Identity::new("t".into());
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&identity.super_properties)
            .unwrap();
        let value: Value = serde_json::from_slice(&decoded).unwrap();
        let keys: Vec<&str> = value.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, CAPTURED_KEYS);
        assert_eq!(value["browser_user_agent"], USER_AGENT);
    }

    #[test]
    fn accept_language_matches_capture() {
        assert_eq!(accept_language("pt-BR"), "pt-BR,pt;q=0.9,en-US;q=0.8,en;q=0.7");
    }
}
