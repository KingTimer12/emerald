//! The member list's search box. Results come from the gateway as chunks
//! tagged with the nonce of the search that asked for them, so a slow answer
//! to an older query never replaces a newer one.

use emerald_discord::model::Snowflake;

#[derive(Default)]
pub struct MemberSearch {
    query: String,
    /// Counts searches; the latest one is the only one whose answer counts.
    generation: u64,
    results: Vec<Snowflake>,
    /// The current query was sent and has not been answered.
    waiting: bool,
}

impl MemberSearch {
    pub fn is_active(&self) -> bool {
        !self.query.is_empty()
    }

    pub fn is_waiting(&self) -> bool {
        self.waiting
    }

    pub fn results(&self) -> &[Snowflake] {
        &self.results
    }

    /// Replaces the query, or ends the search when it is blank. Returns the
    /// generation the query's request must be tagged with, `None` if
    /// nothing has changed or there is nothing to search for.
    pub fn set_query(&mut self, text: &str) -> Option<u64> {
        let query = text.trim().to_lowercase();
        if query == self.query {
            return None;
        }
        self.generation += 1;
        self.results.clear();
        self.waiting = !query.is_empty();
        self.query = query;
        self.waiting.then_some(self.generation)
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    /// Whether `generation` is still the search on screen.
    pub fn is_current(&self, generation: u64) -> bool {
        self.is_active() && self.generation == generation
    }

    /// Takes a chunk's members if its nonce belongs to the current search.
    /// Returns whether it did.
    pub fn accept(&mut self, nonce: Option<&str>, members: impl Iterator<Item = Snowflake>) -> bool {
        let Some(generation) = nonce.and_then(|n| n.strip_prefix(NONCE_PREFIX)?.parse::<u64>().ok()) else {
            return false;
        };
        if !self.is_current(generation) {
            return false;
        }
        for id in members {
            if !self.results.contains(&id) {
                self.results.push(id);
            }
        }
        self.waiting = false;
        true
    }
}

const NONCE_PREFIX: &str = "search-";

pub fn nonce(generation: u64) -> String {
    format!("{NONCE_PREFIX}{generation}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_latest_search_is_answered() {
        let mut search = MemberSearch::default();
        let first = search.set_query("al").unwrap();
        let second = search.set_query("ali").unwrap();
        assert!(!search.accept(Some(&nonce(first)), [Snowflake(1)].into_iter()));
        assert!(search.accept(Some(&nonce(second)), [Snowflake(2), Snowflake(2)].into_iter()));
        assert_eq!(search.results(), &[Snowflake(2)]);
        assert!(!search.is_waiting());
    }

    #[test]
    fn blank_query_ends_the_search() {
        let mut search = MemberSearch::default();
        let generation = search.set_query(" Bob ").unwrap();
        assert_eq!(search.query(), "bob");
        assert_eq!(search.set_query("bob"), None);
        assert_eq!(search.set_query("  "), None);
        assert!(!search.is_active());
        assert!(!search.accept(Some(&nonce(generation)), std::iter::empty()));
    }

    #[test]
    fn foreign_chunks_are_ignored() {
        let mut search = MemberSearch::default();
        search.set_query("x").unwrap();
        assert!(!search.accept(None, [Snowflake(1)].into_iter()));
        assert!(search.results().is_empty());
        assert!(search.is_waiting());
    }
}
