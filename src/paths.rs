//! Per-user directories. macOS and Linux keep the Unix layout
//! (`~/.config/emerald`); Windows uses the AppData folders.

use std::path::PathBuf;

/// Token and preferences: `~/.config/emerald`, or `%APPDATA%\emerald` on
/// Windows.
pub fn config_dir() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        env("APPDATA")?
    } else {
        env("HOME")?.join(".config")
    };
    Some(base.join("emerald"))
}

/// Disposable data: `~/Library/Caches/emerald` on macOS,
/// `%LOCALAPPDATA%\emerald` on Windows, `$XDG_CACHE_HOME/emerald` (or
/// `~/.cache/emerald`) elsewhere.
pub fn cache_dir() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        env("LOCALAPPDATA")?
    } else if cfg!(target_os = "macos") {
        env("HOME")?.join("Library/Caches")
    } else {
        env("XDG_CACHE_HOME").or_else(|| Some(env("HOME")?.join(".cache")))?
    };
    Some(base.join("emerald"))
}

fn env(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}
