//! The quick switcher's matching: which places answer what was typed, best
//! first, the way Discord's Ctrl+K ranks them. Building the places and
//! drawing them is the window's job.

use emerald_discord::model::Snowflake;

/// Results shown at most, as the web client caps its list.
pub const MAX_RESULTS: usize = 20;

/// Where a result leads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// A channel or thread of the guild `guild`.
    Channel { guild: Snowflake, channel: Snowflake },
    /// A DM or group DM.
    Dm(Snowflake),
    /// A server, opened on its landing channel.
    Guild(Snowflake),
}

/// The kinds a leading sigil narrows the search to, as on Discord: `#`
/// channels, `@` people, `*` servers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Only {
    Channels,
    People,
    Guilds,
}

/// One place the switcher can go to.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub place: Place,
    /// What is matched and shown.
    pub name: String,
    /// The server it is in, shown dimmed.
    pub detail: Option<String>,
    /// Drawn before the name: the channel's icon, or a DM's avatar.
    pub icon: &'static str,
    pub user: Option<Snowflake>,
}

impl Candidate {
    fn is(&self, only: Only) -> bool {
        matches!(
            (only, self.place),
            (Only::Channels, Place::Channel { .. }) | (Only::People, Place::Dm(_)) | (Only::Guilds, Place::Guild(_))
        )
    }
}

/// The query without its sigil, and what the sigil narrows to.
pub fn parse(query: &str) -> (Option<Only>, &str) {
    let query = query.trim_start();
    let only = match query.chars().next() {
        Some('#') => Some(Only::Channels),
        Some('@') => Some(Only::People),
        Some('*') => Some(Only::Guilds),
        _ => None,
    };
    let rest = if only.is_some() { &query[1..] } else { query };
    (only, rest.trim())
}

/// How well `name` answers `query` (lowercase), higher being better; `None`
/// when it does not. A whole match beats a prefix, a prefix beats a word
/// start, which beats text anywhere, which beats letters in order.
pub fn score(name: &str, query: &str) -> Option<u32> {
    if query.is_empty() {
        return Some(0);
    }
    let name = name.to_lowercase();
    if name == query {
        return Some(1000);
    }
    if name.starts_with(query) {
        return Some(800 - shorter_is_better(&name));
    }
    let word_start = name
        .match_indices(query)
        .any(|(ix, _)| name[..ix].ends_with(|c: char| !c.is_alphanumeric()));
    if word_start {
        return Some(600 - shorter_is_better(&name));
    }
    if name.contains(query) {
        return Some(400 - shorter_is_better(&name));
    }
    let mut letters = name.chars();
    query
        .chars()
        .all(|wanted| letters.any(|c| c == wanted))
        .then(|| 200 - shorter_is_better(&name))
}

/// Among equal kinds of match, shorter names are closer to what was typed.
fn shorter_is_better(name: &str) -> u32 {
    name.chars().count().min(100) as u32
}

/// Indices of the candidates matching `query`, best first, at most
/// [`MAX_RESULTS`]. An empty query keeps the given order, which the caller
/// fills with recent places first.
pub fn rank(query: &str, candidates: &[Candidate]) -> Vec<usize> {
    let (only, rest) = parse(query);
    let rest = rest.to_lowercase();
    let mut found: Vec<(u32, usize)> = candidates
        .iter()
        .enumerate()
        .filter(|(_, c)| only.is_none_or(|only| c.is(only)))
        .filter_map(|(ix, c)| Some((score(&c.name, &rest)?, ix)))
        .collect();
    // Stable, so ties keep the caller's order (recent places first).
    found.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    found.into_iter().take(MAX_RESULTS).map(|(_, ix)| ix).collect()
}

/// Channels opened lately, newest first, to offer before anything is typed.
#[derive(Default)]
pub struct Recent {
    channels: Vec<Snowflake>,
}

impl Recent {
    const KEEP: usize = 10;

    pub fn visit(&mut self, channel: Snowflake) {
        self.channels.retain(|c| *c != channel);
        self.channels.insert(0, channel);
        self.channels.truncate(Self::KEEP);
    }

    /// Where `channel` sits among the recent ones, `None` if it is not one.
    pub fn position(&self, channel: Snowflake) -> Option<usize> {
        self.channels.iter().position(|c| *c == channel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn channel(id: u64, name: &str) -> Candidate {
        Candidate {
            place: Place::Channel {
                guild: Snowflake(1),
                channel: Snowflake(id),
            },
            ..candidate(name)
        }
    }

    fn candidate(name: &str) -> Candidate {
        Candidate {
            place: Place::Guild(Snowflake(0)),
            name: name.into(),
            detail: None,
            icon: "",
            user: None,
        }
    }

    #[test]
    fn closer_matches_rank_first() {
        let candidates = [
            channel(1, "off-topic"),
            channel(2, "general-chat"),
            channel(3, "general"),
            channel(4, "the-general-store"),
            channel(5, "gamer-entrance-lounge"),
        ];
        let names = |query| rank(query, &candidates).into_iter().map(|ix| candidates[ix].name.as_str()).collect::<Vec<_>>();
        assert_eq!(
            names("general"),
            ["general", "general-chat", "the-general-store"]
        );
        // Letters in order still find it, below anything closer.
        assert_eq!(names("gnrl"), ["general", "general-chat", "the-general-store", "gamer-entrance-lounge"]);
        assert_eq!(names("GEN").first(), Some(&"general"));
    }

    #[test]
    fn sigils_narrow_the_kind() {
        let candidates = [
            channel(1, "rust"),
            Candidate {
                place: Place::Guild(Snowflake(9)),
                ..candidate("Rust Brasil")
            },
            Candidate {
                place: Place::Dm(Snowflake(7)),
                ..candidate("rustacean")
            },
        ];
        assert_eq!(rank("rust", &candidates).len(), 3);
        assert_eq!(rank("#rust", &candidates), [0]);
        assert_eq!(rank("* rust", &candidates), [1]);
        assert_eq!(rank("@rust", &candidates), [2]);
        assert_eq!(parse("  #general "), (Some(Only::Channels), "general"));
    }

    #[test]
    fn an_empty_query_keeps_the_given_order() {
        let candidates: Vec<Candidate> = (0..30).map(|id| channel(id, "c")).collect();
        let shown = rank("", &candidates);
        assert_eq!(shown.len(), MAX_RESULTS);
        assert_eq!(&shown[..3], [0, 1, 2]);
    }

    #[test]
    fn recent_places_move_to_the_front() {
        let mut recent = Recent::default();
        for id in 1..=12 {
            recent.visit(Snowflake(id));
        }
        recent.visit(Snowflake(5));
        assert_eq!(recent.position(Snowflake(5)), Some(0));
        assert_eq!(recent.position(Snowflake(12)), Some(1));
        // Only the last ten are kept.
        assert_eq!(recent.position(Snowflake(2)), None);
    }
}
