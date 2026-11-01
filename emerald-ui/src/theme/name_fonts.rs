//! The fonts people pick for their display name. Discord ships its own
//! copies; Emerald bundles the open-licence ones (SIL Open Font License,
//! each beside its `*-OFL.txt` in `assets/fonts/names/`) and draws the
//! others in the interface font. A face is handed to the text system the
//! first time a name asks for it, so unused ones cost no memory.

use std::sync::atomic::{AtomicU32, Ordering};

/// Each font id with a bundled face: the family it installs and its bytes.
const FACES: &[(u32, &str, &[u8])] = &[
    (1, "Bangers", include_bytes!("../../assets/fonts/names/Bangers.ttf")),
    (2, "BioRhyme", include_bytes!("../../assets/fonts/names/BioRhyme.ttf")),
    (3, "Cherry Bomb One", include_bytes!("../../assets/fonts/names/CherryBombOne.ttf")),
    (4, "Chicle", include_bytes!("../../assets/fonts/names/Chicle.ttf")),
    (6, "MuseoModerno", include_bytes!("../../assets/fonts/names/MuseoModerno.ttf")),
    (8, "Pixelify Sans", include_bytes!("../../assets/fonts/names/PixelifySans.ttf")),
    (12, "Zilla Slab", include_bytes!("../../assets/fonts/names/ZillaSlab.ttf")),
    (13, "Playpen Sans", include_bytes!("../../assets/fonts/names/PlaypenSans.ttf")),
    (14, "Orbitron", include_bytes!("../../assets/fonts/names/Orbitron.ttf")),
    (15, "New Rocker", include_bytes!("../../assets/fonts/names/NewRocker.ttf")),
    (16, "Kalam", include_bytes!("../../assets/fonts/names/Kalam.ttf")),
];

/// Font ids whose face was handed to the text system, one bit each.
static LOADED: AtomicU32 = AtomicU32::new(0);

/// The family for display-name font `font_id`, or `None` for the default
/// font, an unknown id, or one whose font is not open-licensed (Compagnon,
/// Neo Castel, Ribes, Sinistre and Hexagon). Its face is loaded the first
/// time a name asks for it.
pub fn use_family(font_id: u32, cx: &gpui::App) -> Option<&'static str> {
    let (id, family, bytes) = FACES.iter().find(|(id, ..)| *id == font_id)?;
    let bit = 1 << id;
    if LOADED.fetch_or(bit, Ordering::Relaxed) & bit == 0
        && let Err(e) = cx.text_system().add_fonts(vec![std::borrow::Cow::Borrowed(*bytes)])
    {
        tracing::warn!("name font {id} not loaded: {e}");
        return None;
    }
    Some(family)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn family(font_id: u32) -> Option<&'static str> {
        FACES.iter().find(|(id, ..)| *id == font_id).map(|(_, family, _)| *family)
    }

    #[test]
    fn ids_map_to_open_fonts_or_none() {
        assert_eq!(family(4), Some("Chicle"));
        assert_eq!(family(7), None);
        assert_eq!(family(11), None);
        assert_eq!(family(999), None);
    }

    #[test]
    fn each_face_installs_the_family_it_is_listed_under() {
        for (id, family, bytes) in FACES {
            let face = ttf_parser::Face::parse(bytes, 0).unwrap();
            let names: Vec<String> = face
                .names()
                .into_iter()
                .filter(|name| name.is_unicode())
                .filter(|name| [ttf_parser::name_id::TYPOGRAPHIC_FAMILY, ttf_parser::name_id::FAMILY].contains(&name.name_id))
                .filter_map(|name| name.to_string())
                .collect();
            assert!(names.iter().any(|name| name == family), "font {id}: {names:?}");
        }
    }
}
