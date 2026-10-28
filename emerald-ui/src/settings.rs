//! Local preferences, stored as JSON next to the token in
//! `settings.json` under `paths::config_dir`. Unknown or missing fields fall back to
//! defaults, so older files keep loading.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use emerald_discord::model::{Snowflake, Status};

/// Message text sizes offered in Appearance, as in Discord's font scaling.
pub const FONT_SIZES: [f32; 6] = [12., 14., 15., 16., 18., 20.];

/// "Theme" in Appearance: the dark or light palette, or the system's choice.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeChoice {
    #[default]
    Dark,
    Light,
    System,
}

impl ThemeChoice {
    pub const ALL: [ThemeChoice; 3] = [ThemeChoice::Dark, ThemeChoice::Light, ThemeChoice::System];

    /// Whether this choice draws the light palette, given the system's.
    pub fn is_light(self, system_light: bool) -> bool {
        match self {
            ThemeChoice::Dark => false,
            ThemeChoice::Light => true,
            ThemeChoice::System => system_light,
        }
    }
}

/// The window's last place and size, restored on the next launch.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowFrame {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub maximized: bool,
}

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

/// "Voice" in the settings, and the mute and deafen switches, which stay
/// set between calls and launches as in the web client.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceSettings {
    pub muted: bool,
    pub deafened: bool,
    /// Device ids; `None` follows the system default.
    pub input: Option<String>,
    pub output: Option<String>,
    /// Percent, up to 200.
    pub input_volume: u16,
    pub output_volume: u16,
    pub noise_suppression: bool,
    /// Sound under this many decibels is not sent.
    pub sensitivity: i16,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        let defaults = emerald_discord::VoiceControls::default();
        Self {
            muted: false,
            deafened: false,
            input: None,
            output: None,
            input_volume: defaults.input_volume,
            output_volume: defaults.output_volume,
            noise_suppression: defaults.noise_suppression,
            sensitivity: defaults.sensitivity,
        }
    }
}

impl VoiceSettings {
    /// What the voice engine is told.
    pub fn controls(&self) -> emerald_discord::VoiceControls {
        emerald_discord::VoiceControls {
            muted: self.muted,
            deafened: self.deafened,
            input: self.input.clone(),
            output: self.output.clone(),
            input_volume: self.input_volume,
            output_volume: self.output_volume,
            noise_suppression: self.noise_suppression,
            sensitivity: self.sensitivity,
        }
    }
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
    pub theme: ThemeChoice,
    /// Animated emoji, avatars and embeds stay on their first frame.
    pub reduced_motion: bool,
    /// Interface zoom in percent, one of `zoom::LEVELS` when picked in the settings.
    pub zoom: u32,
    /// Keyboard shortcuts the user changed, by `Shortcut::id`, as keystrokes
    /// such as `secondary-k`. Unset ones keep their default key.
    pub shortcuts: BTreeMap<String, String>,
    /// Where the window was when it last closed.
    pub window: Option<WindowFrame>,
    /// The channel open when Emerald last ran, reopened on the next launch.
    pub last_channel: Option<Snowflake>,
    pub voice: VoiceSettings,
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
            theme: ThemeChoice::Dark,
            reduced_motion: false,
            zoom: 100,
            shortcuts: BTreeMap::new(),
            window: None,
            last_channel: None,
            voice: VoiceSettings::default(),
        }
    }
}

impl Settings {
    /// The interface language these settings ask for.
    pub fn language(&self) -> &str {
        self.language.as_deref().unwrap_or_else(|| emerald_core::i18n::system())
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
    Some(emerald_core::paths::config_dir()?.join("settings.json"))
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
