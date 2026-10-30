//! Keyboard shortcuts the user can change under Settings > Shortcuts. Each
//! has a default key; a keystroke the user saved (such as `secondary-k`)
//! replaces it. The editing keys of text inputs and Escape stay fixed.

use std::collections::BTreeMap;

use gpui::{App, KeyBinding, Keystroke, actions};

actions!(
    emerald,
    [QuickSwitch, EditLast, MarkRead, NextUnread, PreviousChannel, NextChannel, GoBack, GoForward, Quit, Logout]
);

/// Keys the text inputs and the window use; no shortcut may take them.
const FIXED: [&str; 10] = [
    "escape",
    "enter",
    "tab",
    "backspace",
    "delete",
    "shift-enter",
    "secondary-a",
    "secondary-c",
    "secondary-v",
    "secondary-x",
];

/// A shortcut the user can rebind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shortcut {
    QuickSwitch,
    EditLast,
    MarkRead,
    NextUnread,
    PreviousChannel,
    NextChannel,
    /// Back to the place opened before, as the title bar's arrow.
    GoBack,
    GoForward,
    Quit,
    Logout,
}

impl Shortcut {
    pub const ALL: [Shortcut; 10] = [
        Shortcut::QuickSwitch,
        Shortcut::EditLast,
        Shortcut::MarkRead,
        Shortcut::NextUnread,
        Shortcut::PreviousChannel,
        Shortcut::NextChannel,
        Shortcut::GoBack,
        Shortcut::GoForward,
        Shortcut::Quit,
        Shortcut::Logout,
    ];

    /// The key in `Settings::shortcuts` and the translation key suffix.
    pub fn id(self) -> &'static str {
        match self {
            Shortcut::QuickSwitch => "quick_switch",
            Shortcut::EditLast => "edit_last",
            Shortcut::MarkRead => "mark_read",
            Shortcut::NextUnread => "next_unread",
            Shortcut::PreviousChannel => "previous_channel",
            Shortcut::NextChannel => "next_channel",
            Shortcut::GoBack => "go_back",
            Shortcut::GoForward => "go_forward",
            Shortcut::Quit => "quit",
            Shortcut::Logout => "log_out",
        }
    }

    /// The Discord-like key, as the web client has it, or a free one.
    pub fn default_key(self) -> &'static str {
        match self {
            Shortcut::QuickSwitch => "secondary-k",
            Shortcut::EditLast => "up",
            Shortcut::MarkRead => "shift-escape",
            Shortcut::NextUnread => "alt-shift-down",
            Shortcut::PreviousChannel => "alt-up",
            Shortcut::NextChannel => "alt-down",
            // The desktop client's history keys.
            Shortcut::GoBack if cfg!(target_os = "macos") => "cmd-[",
            Shortcut::GoForward if cfg!(target_os = "macos") => "cmd-]",
            Shortcut::GoBack => "alt-left",
            Shortcut::GoForward => "alt-right",
            Shortcut::Quit => "secondary-q",
            Shortcut::Logout => "secondary-shift-l",
        }
    }

    /// The key in use: the user's saved one if it is valid, else the default.
    pub fn key(self, overrides: &BTreeMap<String, String>) -> String {
        overrides
            .get(self.id())
            .filter(|key| normalize(key).is_some())
            .cloned()
            .unwrap_or_else(|| self.default_key().to_owned())
    }

    fn binding(self, key: &str) -> KeyBinding {
        match self {
            Shortcut::QuickSwitch => KeyBinding::new(key, QuickSwitch, Some("Emerald")),
            Shortcut::EditLast => KeyBinding::new(key, EditLast, Some("Emerald")),
            Shortcut::MarkRead => KeyBinding::new(key, MarkRead, Some("Emerald")),
            Shortcut::NextUnread => KeyBinding::new(key, NextUnread, Some("Emerald")),
            Shortcut::PreviousChannel => KeyBinding::new(key, PreviousChannel, Some("Emerald")),
            Shortcut::NextChannel => KeyBinding::new(key, NextChannel, Some("Emerald")),
            Shortcut::GoBack => KeyBinding::new(key, GoBack, Some("Emerald")),
            Shortcut::GoForward => KeyBinding::new(key, GoForward, Some("Emerald")),
            Shortcut::Quit => KeyBinding::new(key, Quit, None),
            Shortcut::Logout => KeyBinding::new(key, Logout, None),
        }
    }
}

/// Why a keystroke cannot be given to a shortcut.
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Not a keystroke gpui understands.
    Invalid,
    /// Text editing or Escape.
    Fixed,
    /// Another shortcut has it.
    Taken(Shortcut),
}

/// Checks `key` for `shortcut` against the fixed keys and the other
/// shortcuts' keys in use.
pub fn check(shortcut: Shortcut, key: &str, overrides: &BTreeMap<String, String>) -> Result<String, Refusal> {
    let key = normalize(key).ok_or(Refusal::Invalid)?;
    if FIXED.iter().any(|fixed| normalize(fixed).as_deref() == Some(key.as_str())) {
        return Err(Refusal::Fixed);
    }
    for other in Shortcut::ALL.into_iter().filter(|other| *other != shortcut) {
        if normalize(&other.key(overrides)).as_deref() == Some(key.as_str()) {
            return Err(Refusal::Taken(other));
        }
    }
    Ok(key)
}

/// The canonical text of a keystroke, so `cmd-k` and `secondary-k` compare
/// equal on macOS; `None` when gpui does not parse it or it names no key.
pub fn normalize(key: &str) -> Option<String> {
    Keystroke::parse(key)
        .ok()
        .filter(|stroke| !stroke.key.is_empty() && !stroke.key.contains(char::is_whitespace))
        .map(|stroke| stroke.unparse())
}

/// A keystroke as the user reads it: "Cmd+K", "Alt+Down".
pub fn describe(key: &str) -> String {
    let Ok(stroke) = Keystroke::parse(key) else { return key.to_owned() };
    let modifiers = stroke.modifiers;
    let mac = cfg!(target_os = "macos");
    let mut parts = Vec::new();
    if modifiers.control {
        parts.push("Ctrl".to_owned());
    }
    if modifiers.alt {
        parts.push(if mac { "Option" } else { "Alt" }.to_owned());
    }
    if modifiers.shift {
        parts.push("Shift".to_owned());
    }
    if modifiers.platform {
        parts.push(if mac { "Cmd" } else { "Win" }.to_owned());
    }
    let mut key_name = stroke.key.chars();
    let name: String = match key_name.next() {
        Some(first) => first.to_uppercase().chain(key_name).collect(),
        None => String::new(),
    };
    parts.push(name);
    parts.join("+")
}

/// Binds the fixed keys and every shortcut, the user's keys where saved.
/// Replaces all bindings, so it runs again when a shortcut changes.
pub fn bind(cx: &mut App, overrides: &BTreeMap<String, String>) {
    cx.clear_key_bindings();
    crate::text_input::bind_keys(cx);
    crate::app::bind_fixed_keys(cx);
    cx.bind_keys(Shortcut::ALL.map(|shortcut| shortcut.binding(&shortcut.key(overrides))));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn back_and_forward_use_the_desktop_client_keys() {
        let (back, forward) = if cfg!(target_os = "macos") { ("cmd-[", "cmd-]") } else { ("alt-left", "alt-right") };
        assert_eq!(Shortcut::GoBack.default_key(), back);
        assert_eq!(Shortcut::GoForward.default_key(), forward);
        assert!(Shortcut::ALL.contains(&Shortcut::GoBack));
    }

    #[test]
    fn defaults_are_valid_and_distinct() {
        let none = BTreeMap::new();
        for shortcut in Shortcut::ALL {
            assert!(normalize(shortcut.default_key()).is_some(), "{shortcut:?}");
            assert_eq!(shortcut.key(&none), shortcut.default_key());
        }
        let keys: Vec<_> = Shortcut::ALL.iter().filter_map(|s| normalize(s.default_key())).collect();
        let mut unique = keys.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), keys.len());
    }

    #[test]
    fn a_saved_key_replaces_the_default_but_bad_ones_do_not() {
        let mut overrides = BTreeMap::new();
        overrides.insert("next_channel".to_owned(), "alt-j".to_owned());
        overrides.insert("previous_channel".to_owned(), "foo-bar".to_owned());
        overrides.insert("edit_last".to_owned(), "alt-not a key".to_owned());
        assert_eq!(Shortcut::NextChannel.key(&overrides), "alt-j");
        assert_eq!(Shortcut::PreviousChannel.key(&overrides), "alt-up");
        assert_eq!(Shortcut::EditLast.key(&overrides), "up");
    }

    #[test]
    fn fixed_and_taken_keys_are_refused() {
        let overrides = BTreeMap::new();
        assert_eq!(check(Shortcut::NextChannel, "escape", &overrides), Err(Refusal::Fixed));
        assert_eq!(check(Shortcut::NextChannel, "alt-up", &overrides), Err(Refusal::Taken(Shortcut::PreviousChannel)));
        assert_eq!(check(Shortcut::NextChannel, "no-such-key-here", &overrides), Err(Refusal::Invalid));
        assert!(check(Shortcut::NextChannel, "alt-j", &overrides).is_ok());
        // A shortcut may keep its own key.
        assert!(check(Shortcut::PreviousChannel, "alt-up", &overrides).is_ok());
    }

    #[test]
    fn keystrokes_read_back_for_people() {
        assert_eq!(describe("alt-down"), if cfg!(target_os = "macos") { "Option+Down" } else { "Alt+Down" });
        assert_eq!(describe("shift-escape"), "Shift+Escape");
        assert_eq!(describe("a"), "A");
    }
}
