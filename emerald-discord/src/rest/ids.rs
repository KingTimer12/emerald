//! Ids the client makes (nonces) and codes it reads from what users paste.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::model::DISCORD_EPOCH_MS;

/// Snowflake-shaped nonce, like the web client's. The low bits carry a
/// counter so two sends in the same millisecond still differ.
pub fn nonce() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed) & 0x3f_ffff;
    (ms.saturating_sub(DISCORD_EPOCH_MS) << 22 | count).to_string()
}

/// Extracts the invite code from what a user pastes: a bare code,
/// `discord.gg/<code>`, or `discord.com/invite/<code>`, with or without
/// scheme, trailing slash or query string.
pub fn invite_code(input: &str) -> Option<String> {
    let input = input.trim();
    let input = input.split(['?', '#']).next().unwrap_or_default();
    let code = input.trim_end_matches('/').rsplit('/').next()?;
    let valid = !code.is_empty() && code.len() <= 64 && code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    valid.then(|| code.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn nonces_differ_within_a_millisecond() {
        assert_ne!(nonce(), nonce());
    }
}
