//! The interface font. Discord draws gg sans (Whitney before 2021); both are
//! licensed, so Emerald bundles Figtree (SIL Open Font License, licence in
//! `assets/fonts/OFL.txt`), the closest free match found when compared against
//! the 2026 screenshots: same round single-storey "a", x-height and width.
//! Noto Sans ran wider and Nunito Sans rounder.

/// The four weights the interface uses (regular, medium, semibold and
/// bold), each upright and italic.
pub const BUNDLED: &[&[u8]] = &[
    include_bytes!("../../assets/fonts/Figtree-Regular.ttf"),
    include_bytes!("../../assets/fonts/Figtree-Medium.ttf"),
    include_bytes!("../../assets/fonts/Figtree-SemiBold.ttf"),
    include_bytes!("../../assets/fonts/Figtree-Bold.ttf"),
    include_bytes!("../../assets/fonts/Figtree-Italic.ttf"),
    include_bytes!("../../assets/fonts/Figtree-MediumItalic.ttf"),
    include_bytes!("../../assets/fonts/Figtree-SemiBoldItalic.ttf"),
    include_bytes!("../../assets/fonts/Figtree-BoldItalic.ttf"),
];

/// The interface font's family for the style in use.
pub fn sans() -> &'static str {
    match super::style() {
        crate::settings::Style::Y2026 => "Figtree",
    }
}

/// Hands the bundled faces to the text system. A face it cannot read only
/// costs the look: the window falls back to the system font.
pub fn load(cx: &gpui::App) {
    let faces = BUNDLED.iter().map(|bytes| std::borrow::Cow::Borrowed(*bytes)).collect();
    if let Err(e) = cx.text_system().add_fonts(faces) {
        tracing::warn!("bundled font not loaded: {e}");
    }
}

/// Code blocks: the system's monospace face, as the web client falls back
/// to when its own font is missing.
#[cfg(target_os = "macos")]
pub const MONO: &str = "Menlo";
#[cfg(not(target_os = "macos"))]
pub const MONO: &str = "Consolas";

#[cfg(test)]
mod tests {
    use super::*;

    /// The family a face is installed under: its typographic family when it
    /// has one (static weights call their legacy family "Figtree Medium").
    fn family(bytes: &[u8]) -> String {
        let face = ttf_parser::Face::parse(bytes, 0).unwrap();
        let names: Vec<_> = face.names().into_iter().filter(|name| name.is_unicode()).collect();
        [ttf_parser::name_id::TYPOGRAPHIC_FAMILY, ttf_parser::name_id::FAMILY]
            .into_iter()
            .find_map(|id| names.iter().find(|name| name.name_id == id).and_then(|name| name.to_string()))
            .unwrap()
    }

    #[test]
    fn bundled_font_family_matches_the_name_the_ui_asks_for() {
        for bytes in BUNDLED {
            assert_eq!(family(bytes), sans());
        }
    }

    /// Markdown's `*x*` and the "(edited)" mark ask for italics; without an
    /// italic face the text would draw upright.
    #[test]
    fn bundled_font_has_italics_for_each_weight() {
        let faces: Vec<_> = BUNDLED.iter().map(|bytes| ttf_parser::Face::parse(bytes, 0).unwrap()).collect();
        for weight in [400, 500, 600, 700] {
            assert!(
                faces.iter().any(|face| face.is_italic() && face.weight().to_number() == weight),
                "no italic at {weight}"
            );
        }
    }

    #[test]
    fn bundled_font_covers_portuguese() {
        let face = ttf_parser::Face::parse(BUNDLED[0], 0).unwrap();
        for ch in "ãçéêíóõúÁÉÍÓÚ…•—".chars() {
            assert!(face.glyph_index(ch).is_some(), "missing {ch}");
        }
    }
}
