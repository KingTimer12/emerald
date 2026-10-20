//! Local preferences, stored as JSON next to the token in
//! `settings.json` under `paths::config_dir`. Unknown or missing fields fall back to
//! defaults, so older files keep loading.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::discord::model::Status;

/// Message text sizes offered in Appearance, as in Discord's font scaling.
pub const FONT_SIZES: [f32; 6] = [12., 14., 15., 16., 18., 20.];

/// "Time format" in Language & Time: the language's own clock, or a fixed one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimeFormat {
    #[default]
    Auto,
    #[serde(rename = "12h")]
    Hours12,
    #[serde(rename = "24h")]
    Hours24,
}

impl TimeFormat {
    pub const ALL: [TimeFormat; 3] = [TimeFormat::Auto, TimeFormat::Hours12, TimeFormat::Hours24];
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub font_size: f32,
    pub compact: bool,
    /// `None` until the user picks one; the account's saved status applies.
    pub status: Option<Status>,
    /// Profile panel beside 1:1 DMs.
    pub show_profile: bool,
    /// Member list beside guild channels.
    pub show_members: bool,
    /// Interface language as a Discord locale code; `None` follows the system.
    pub language: Option<String>,
    pub time_format: TimeFormat,
    /// "Frequently Used" in the reaction picker, newest first: Unicode
    /// emoji as text, custom ones as `name:id`.
    pub recent_emojis: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            font_size: 16.,
            compact: false,
            status: None,
            show_profile: true,
            show_members: true,
            language: None,
            time_format: TimeFormat::Auto,
            recent_emojis: Vec::new(),
        }
    }
}

impl Settings {
    /// The interface language these settings ask for.
    pub fn language(&self) -> &str {
        self.language.as_deref().unwrap_or_else(|| crate::i18n::system())
    }

    pub fn load() -> Self {
        path()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = path().ok_or(std::io::ErrorKind::NotFound)?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(tmp, path)
    }
}

fn path() -> Option<PathBuf> {
    Some(crate::paths::config_dir()?.join("settings.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_file_keeps_defaults() {
        let settings: Settings = serde_json::from_str(r#"{"compact":true}"#).unwrap();
        assert!(settings.compact);
        assert_eq!(settings.font_size, 16.);
        assert_eq!(settings.status, None);
    }

    #[test]
    fn status_round_trips_lowercase() {
        let settings = Settings {
            status: Some(Status::Dnd),
            ..Settings::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains(r#""status":"dnd""#));
        assert_eq!(serde_json::from_str::<Settings>(&json).unwrap(), settings);
    }
}
