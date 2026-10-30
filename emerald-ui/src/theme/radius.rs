//! Corner radii by what they round, so each style sets its own. Measured on
//! Discord 2026 (light theme screenshots in `themes/2026`).

use gpui::Pixels;

use super::style;
use crate::settings::Style;
use crate::zoom::zoomed;

/// One radius per style, zoomed with the interface.
fn per_style(y2026: f32) -> Pixels {
    zoomed(match style() {
        Style::Y2026 => y2026,
    })
}

/// App and server tags, small chips and badges.
pub fn tag() -> Pixels {
    per_style(4.)
}

/// Buttons, inputs, search fields, list rows, tabs and the hover toolbar.
pub fn control() -> Pixels {
    per_style(8.)
}

/// Embeds, attachments, cards inside a page, the user panel and the panel corner.
pub fn card() -> Pixels {
    per_style(8.)
}

/// Modals, the settings window, popouts, menus and pickers.
pub fn modal() -> Pixels {
    per_style(12.)
}

/// Server icons and the home button at their usual 40px.
pub fn server_icon() -> Pixels {
    server_icon_at(40.)
}

/// A server icon drawn `size` px wide keeps the 40px icon's proportion.
pub fn server_icon_at(size: f32) -> Pixels {
    per_style(12. * size / 40.)
}

/// An open server folder.
pub fn folder() -> Pixels {
    per_style(16.)
}

/// The rounded end of the unread and selection pills at a list's edge.
pub fn pill() -> Pixels {
    per_style(4.)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use gpui::px;

    use super::*;
    use crate::settings::Style;

    #[test]
    fn the_2026_radii_match_the_screenshots() {
        let _switches = crate::theme::lock_switches();
        crate::zoom::set(100);
        crate::theme::set_style(Style::Y2026);
        assert_eq!(control(), px(8.));
        assert_eq!(server_icon(), px(12.));
        assert_eq!(modal(), px(12.));
        assert_eq!(tag(), px(4.));
    }

    /// Every `.rs` file under `dir`, with its text.
    fn walk(dir: &Path) -> Vec<(PathBuf, String)> {
        let mut files = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(walk(&path));
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                files.push((path, text));
            }
        }
        files
    }

    /// Guards the migration: screens use the style's radii, not gpui's fixed
    /// ones. `rounded_full()` stays: it means a circle in every style.
    #[test]
    fn screens_do_not_use_fixed_radii() {
        let offenders: Vec<String> = walk(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src")))
            .into_iter()
            .filter(|(path, _)| !path.ends_with("radius.rs"))
            .flat_map(|(path, text)| {
                let mut found = Vec::new();
                for (ix, _) in text.match_indices(".rounded") {
                    let rest = &text[ix..];
                    let call = &rest[..rest.find(')').map_or(rest.len(), |end| end + 1)];
                    let allowed = call == ".rounded_full()" || call.contains("radius::");
                    if !allowed {
                        found.push(format!("{}: {call}", path.display()));
                    }
                }
                found
            })
            .collect();
        assert!(offenders.is_empty(), "fixed radii: {offenders:#?}");
    }
}
