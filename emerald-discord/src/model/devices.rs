//! `GET /auth/sessions`: the devices signed in to the account.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AuthSessions {
    #[serde(default)]
    pub user_sessions: Vec<AuthSession>,
}

/// One signed-in device. Discord names it by a hash; the current one's
/// hash comes in READY.
#[derive(Clone, Debug, Deserialize)]
pub struct AuthSession {
    pub id_hash: String,
    /// ISO 8601, as Discord writes it.
    #[serde(default)]
    pub approx_last_used_time: Option<String>,
    #[serde(default)]
    pub client_info: ClientInfo,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ClientInfo {
    /// "Mac OS X", "Windows", "Android", …
    #[serde(default)]
    pub os: Option<String>,
    /// "Discord Client", "Chrome", …
    #[serde(default)]
    pub platform: Option<String>,
    /// A city and country, roughly.
    #[serde(default)]
    pub location: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sessions() {
        let sessions: AuthSessions = serde_json::from_str(
            r#"{"user_sessions":[{"id_hash":"abc","approx_last_used_time":"2025-01-02T03:04:05+00:00",
                "client_info":{"os":"Windows","platform":"Chrome","location":"São Paulo, Brazil"}},
                {"id_hash":"def"}]}"#,
        )
        .unwrap();
        assert_eq!(sessions.user_sessions[0].client_info.os.as_deref(), Some("Windows"));
        assert!(sessions.user_sessions[1].approx_last_used_time.is_none());
    }
}
