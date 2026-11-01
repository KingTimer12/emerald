//! The "Forward To" list: which channels a message can go to, in the web
//! client's order. Building the destinations and drawing them is the
//! window's job.

use emerald_discord::model::Snowflake;

use super::switcher;

/// Destinations picked at most.
pub const MAX_PICKED: usize = 5;
/// Rows listed before anything is typed, as the web client cuts its list.
pub const MAX_LISTED: usize = 15;

/// A channel a message can be forwarded to: an existing DM, group DM, or a
/// guild channel the user can send in. Never a person without a DM, since
/// opening one is a write.
#[derive(Clone, Debug)]
pub struct Dest {
    pub channel: Snowflake,
    pub name: String,
    /// The server it is in.
    pub detail: Option<String>,
    pub icon: &'static str,
    pub user: Option<Snowflake>,
    /// Slowmode the user cannot bypass: the optional text is not sent.
    pub slowmode: bool,
}

/// Indices into `all` to list. With no query: the picked ones, then the
/// recently visited ones, then the rest in `all`'s order, without the
/// source channel unless picked, cut to [`MAX_LISTED`]. With a query: the
/// names that match it, best first.
pub fn destinations(query: &str, picked: &[Snowflake], recent: &[Snowflake], source: Snowflake, all: &[Dest]) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    if !query.is_empty() {
        let mut found: Vec<(u32, usize)> = all
            .iter()
            .enumerate()
            .filter_map(|(ix, d)| Some((switcher::score(&d.name, &query)?, ix)))
            .collect();
        found.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
        return found.into_iter().take(switcher::MAX_RESULTS).map(|(_, ix)| ix).collect();
    }
    let position = |channel: Snowflake| all.iter().position(|d| d.channel == channel);
    let mut listed: Vec<usize> = Vec::new();
    let ordered = picked
        .iter()
        .copied()
        .chain(recent.iter().copied().filter(|c| *c != source))
        .filter_map(position)
        .chain((0..all.len()).filter(|ix| all[*ix].channel != source));
    for ix in ordered {
        if !listed.contains(&ix) {
            listed.push(ix);
        }
        if listed.len() == MAX_LISTED {
            break;
        }
    }
    listed
}

/// Puts `channel` at the front of the picks, or takes it out. Nothing is
/// added past [`MAX_PICKED`].
pub fn toggle(picked: &mut Vec<Snowflake>, channel: Snowflake) {
    if let Some(ix) = picked.iter().position(|c| *c == channel) {
        picked.remove(ix);
    } else if picked.len() < MAX_PICKED {
        picked.insert(0, channel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dest(id: u64, name: &str) -> Dest {
        Dest {
            channel: Snowflake(id),
            name: name.into(),
            detail: None,
            icon: "",
            user: None,
            slowmode: false,
        }
    }

    #[test]
    fn picked_then_recent_then_the_rest_capped() {
        let all: Vec<Dest> = (0..30).map(|id| dest(id, "c")).collect();
        let listed = destinations("", &[Snowflake(20)], &[Snowflake(7), Snowflake(20), Snowflake(0)], Snowflake(0), &all);
        assert_eq!(&listed[..3], [20, 7, 1]);
        assert_eq!(listed.len(), MAX_LISTED);
        assert!(!listed.contains(&0), "the source channel is left out");
    }

    #[test]
    fn queries_match_names_folded() {
        let all = [dest(1, "General"), dest(2, "memes"), dest(3, "gen-chat")];
        assert_eq!(destinations("GEN", &[], &[], Snowflake(9), &all), [0, 2]);
    }

    #[test]
    fn at_most_five_picks_newest_first() {
        let mut picked = Vec::new();
        for id in 1..=6 {
            toggle(&mut picked, Snowflake(id));
        }
        assert_eq!(picked, [5, 4, 3, 2, 1].map(Snowflake));
        toggle(&mut picked, Snowflake(3));
        assert_eq!(picked.len(), 4);
    }
}
