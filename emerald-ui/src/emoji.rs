//! The Unicode emoji the picker offers, in Discord's categories. The data
//! is Unicode's (CLDR names, GitHub shortcodes), from the `emojis` crate.

use std::sync::OnceLock;

/// Newer emoji are left out: macOS and Windows releases still in use draw
/// them as empty boxes. Discord's own set stops at the same version.
const NEWEST: (u32, u32) = (15, 1);

/// Discord's picker categories, which fold Unicode's "Smileys & Emotion"
/// and "People & Body" into one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    People,
    Nature,
    Food,
    Activities,
    Travel,
    Objects,
    Symbols,
    Flags,
}

impl Category {
    pub const ALL: [Self; 8] = [
        Self::People,
        Self::Nature,
        Self::Food,
        Self::Activities,
        Self::Travel,
        Self::Objects,
        Self::Symbols,
        Self::Flags,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::People => t!("reaction.people"),
            Self::Nature => t!("reaction.nature"),
            Self::Food => t!("reaction.food"),
            Self::Activities => t!("reaction.activities"),
            Self::Travel => t!("reaction.travel"),
            Self::Objects => t!("reaction.objects"),
            Self::Symbols => t!("reaction.symbols"),
            Self::Flags => t!("reaction.flags"),
        }
    }

    fn of(group: emojis::Group) -> Self {
        use emojis::Group;
        match group {
            Group::SmileysAndEmotion | Group::PeopleAndBody => Self::People,
            Group::AnimalsAndNature => Self::Nature,
            Group::FoodAndDrink => Self::Food,
            Group::Activities => Self::Activities,
            Group::TravelAndPlaces => Self::Travel,
            Group::Objects => Self::Objects,
            Group::Symbols => Self::Symbols,
            Group::Flags => Self::Flags,
        }
    }
}

/// Every offered emoji per category, in Unicode order.
pub fn by_category() -> &'static [(Category, Vec<&'static emojis::Emoji>)] {
    static CATALOG: OnceLock<Vec<(Category, Vec<&'static emojis::Emoji>)>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let mut catalog: Vec<_> = Category::ALL.iter().map(|c| (*c, Vec::new())).collect();
        for emoji in emojis::iter().filter(|e| offered(e)) {
            let category = Category::of(emoji.group());
            if let Some((_, list)) = catalog.iter_mut().find(|(c, _)| *c == category) {
                list.push(emoji);
            }
        }
        catalog
    })
}

fn offered(emoji: &emojis::Emoji) -> bool {
    let version = emoji.emoji_version();
    (version.major(), version.minor()) <= NEWEST
}

/// The name Discord shows for a Unicode emoji, as in `:thumbsup:`: a
/// worded shortcode over `+1`, and the CLDR name when there is none.
pub fn name(text: &str) -> Option<&'static str> {
    let emoji = emojis::get(text)?;
    let worded = emoji.shortcodes().find(|code| code.starts_with(|c: char| c.is_ascii_alphabetic()));
    Some(worded.or(emoji.shortcode()).unwrap_or(emoji.name()))
}

/// Matches a lowercase `query` against the emoji's shortcodes and name.
pub fn matches(emoji: &emojis::Emoji, query: &str) -> bool {
    emoji.shortcodes().any(|code| code.contains(query)) || emoji.name().contains(query)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories_follow_discord() {
        let catalog = by_category();
        assert_eq!(catalog.len(), 8);
        let people = &catalog[0].1;
        // Smileys come first, people after them, as in Discord's picker.
        assert_eq!(people.first().map(|e| e.as_str()), Some("😀"));
        assert!(people.iter().any(|e| e.as_str() == "👍"));
        assert!(catalog.iter().all(|(_, list)| !list.is_empty()));
    }

    #[test]
    fn names_and_search() {
        assert_eq!(name("👍"), Some("thumbsup"));
        assert_eq!(name("x"), None);
        let thumbs = emojis::get("👍").unwrap();
        assert!(matches(thumbs, "thumbsup"));
        assert!(!matches(thumbs, "heart"));
    }
}
