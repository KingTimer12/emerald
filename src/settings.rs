//! Local preferences, stored as JSON next to the token in
//! `~/.config/emerald/settings.json`. Unknown or missing fields fall back to
//! defaults, so older files keep loading.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::discord::model::Status;

/// Message text sizes offered in Appearance, as in Discord's font scaling.
pub const FONT_SIZES: [f32; 6] = [12., 14., 15., 16., 18., 20.];

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
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            font_size: 16.,
            compact: false,
            status: None,
            show_profile: true,
            show_members: true,
        }
    }
}

impl Settings {
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
    Some(PathBuf::from(std::env::var_os("HOME")?).join(".config/emerald/settings.json"))
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
