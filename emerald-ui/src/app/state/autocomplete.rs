//! The suggestions over the message box: `@` for members and roles, `#`
//! for channels, `:` for emoji and `/` at the start for slash commands, as
//! the web client offers them. What a
//! pick puts in the box reads like Discord's own pills (`@name`, `#name`,
//! `:name:`) and becomes the real mention when the message is sent.

use std::ops::Range;

/// Suggestions shown at once, as in the web client.
pub const LIMIT: usize = 10;
/// `:` waits for this many letters before suggesting emoji, like Discord.
const EMOJI_MIN_QUERY: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    /// `@`: members, roles, `@everyone` and `@here`.
    Mention,
    /// `#`: channels.
    Channel,
    /// `:`: emoji.
    Emoji,
    /// `/` opening the message: slash commands.
    Command,
}

impl Trigger {
    fn of(c: char) -> Option<Self> {
        match c {
            '@' => Some(Self::Mention),
            '#' => Some(Self::Channel),
            ':' => Some(Self::Emoji),
            '/' => Some(Self::Command),
            _ => None,
        }
    }
}

/// The word being completed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Query {
    pub trigger: Trigger,
    /// What follows the trigger, as typed.
    pub text: String,
    /// The trigger and the text, as byte offsets in the box.
    pub range: Range<usize>,
}

/// The word that ends at `cursor` when it starts with a trigger, at the
/// start of the text or after a space, as Discord reads it.
pub fn query_at(text: &str, cursor: usize) -> Option<Query> {
    if cursor > text.len() || !text.is_char_boundary(cursor) {
        return None;
    }
    let before = &text[..cursor];
    let start = before
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace())
        .map_or(0, |(ix, c)| ix + c.len_utf8());
    let word = &before[start..];
    let trigger = Trigger::of(word.chars().next()?)?;
    let typed = &word[1..];
    // Commands only open the message, before any space.
    if trigger == Trigger::Command && start != 0 {
        return None;
    }
    // `:smile:` is already a finished emoji, and `a:b` is not one at all.
    if trigger == Trigger::Emoji && (typed.chars().count() < EMOJI_MIN_QUERY || typed.contains(':')) {
        return None;
    }
    Some(Query {
        trigger,
        text: typed.to_owned(),
        range: start..cursor,
    })
}

/// How well `names` match `query`: 0 when one starts with it, 1 when one
/// contains it, `None` when none does. Case is ignored.
pub fn score<'a>(query: &str, names: impl IntoIterator<Item = &'a str>) -> Option<u8> {
    let query = query.to_lowercase();
    names
        .into_iter()
        .filter_map(|name| {
            let name = name.to_lowercase();
            if name.starts_with(&query) {
                Some(0)
            } else if name.contains(&query) {
                Some(1)
            } else {
                None
            }
        })
        .min()
}

/// The best `LIMIT` of `items` for `query`: prefix matches first, then the
/// rest, each kept in the order given.
pub fn rank<T, I, N>(query: &str, items: I, names: N) -> Vec<T>
where
    I: IntoIterator<Item = T>,
    N: Fn(&T) -> Vec<&str>,
{
    let mut scored: Vec<(u8, usize, T)> = items
        .into_iter()
        .enumerate()
        .filter_map(|(ix, item)| score(query, names(&item)).map(|score| (score, ix, item)))
        .collect();
    scored.sort_by_key(|(score, ix, _)| (*score, *ix));
    scored.into_iter().take(LIMIT).map(|(_, _, item)| item).collect()
}

/// A pick put in the box: the text shown there, and what the message says
/// instead when sent (`<@id>`, `<#id>`, `<:name:id>`), if anything.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Completion {
    pub shown: String,
    pub sent: Option<String>,
}

/// `text` as it goes to Discord: each pick still in it, whole, becomes its
/// mention. Longer picks are tried first, so `@ann` never eats `@anna`.
pub fn expand(text: &str, completions: &[Completion]) -> String {
    let mut picks: Vec<(&str, &str)> = completions
        .iter()
        .filter_map(|c| Some((c.shown.as_str(), c.sent.as_deref()?)))
        .filter(|(shown, _)| !shown.is_empty())
        .collect();
    picks.sort_by_key(|(shown, _)| std::cmp::Reverse(shown.len()));
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    'scan: while let Some(c) = rest.chars().next() {
        for (shown, sent) in &picks {
            if let Some(after) = rest.strip_prefix(shown)
                && !after.chars().next().is_some_and(continues_name)
            {
                out.push_str(sent);
                rest = after;
                continue 'scan;
            }
        }
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// A character that would make a pick part of a longer word.
fn continues_name(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The open suggestion list: the query it answers, how many entries it has
/// and which one is highlighted. The entries themselves are built by the
/// window, which knows the members, channels and emoji.
#[derive(Default)]
pub struct Autocomplete {
    query: Option<Query>,
    len: usize,
    selected: usize,
    /// Escape closed the list for this query; typing on opens it again.
    dismissed: Option<Query>,
}

impl Autocomplete {
    /// Follows the box: the word at the cursor. Returns whether the query
    /// changed, so the entries must be built again.
    pub fn update(&mut self, text: &str, cursor: usize) -> bool {
        let query = query_at(text, cursor);
        if self.dismissed.is_some() && self.dismissed != query {
            self.dismissed = None;
        }
        if query == self.query {
            return false;
        }
        self.query = query;
        self.selected = 0;
        true
    }

    /// The query, while the list is open.
    pub fn query(&self) -> Option<&Query> {
        self.query.as_ref().filter(|q| self.dismissed.as_ref() != Some(q))
    }

    /// The entries were built: `len` of them.
    pub fn set_len(&mut self, len: usize) {
        self.len = len;
        self.selected = self.selected.min(len.saturating_sub(1));
    }

    pub fn is_open(&self) -> bool {
        self.query().is_some() && self.len > 0
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn select(&mut self, ix: usize) {
        if ix < self.len {
            self.selected = ix;
        }
    }

    /// Up (-1) or Down (1), wrapping around as in Discord.
    pub fn step(&mut self, by: isize) {
        if self.len > 0 {
            self.selected = (self.selected as isize + by).rem_euclid(self.len as isize) as usize;
        }
    }

    /// Escape: closed until the word changes.
    pub fn dismiss(&mut self) -> bool {
        if !self.is_open() {
            return false;
        }
        self.dismissed = self.query.clone();
        true
    }

    /// A pick was made or the channel changed.
    pub fn close(&mut self) {
        self.query = None;
        self.len = 0;
        self.selected = 0;
        self.dismissed = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triggers_start_a_word() {
        let q = query_at("hi @an", 6).unwrap();
        assert_eq!((q.trigger, q.text.as_str(), q.range), (Trigger::Mention, "an", 3..6));
        assert_eq!(query_at("@", 1).unwrap().text, "");
        assert_eq!(query_at("#gen", 4).unwrap().trigger, Trigger::Channel);
        // Mid-word, as in an e-mail address, is not a mention.
        assert_eq!(query_at("me@mail", 7), None);
        assert_eq!(query_at("/ro", 3).unwrap().trigger, Trigger::Command);
        // A slash later on is just text.
        assert_eq!(query_at("and/or", 6), None);
        assert_eq!(query_at("a /b", 4), None);
        // Only the word the cursor ends.
        assert_eq!(query_at("@ann hello", 10), None);
        assert_eq!(query_at("line\n@x", 7).unwrap().range, 5..7);
    }

    #[test]
    fn emoji_need_two_letters_and_no_closing_colon() {
        assert_eq!(query_at(":s", 2), None);
        assert_eq!(query_at(":", 1), None);
        assert_eq!(query_at(":smile:", 7), None);
        assert_eq!(query_at("10:30", 5), None);
        assert_eq!(query_at(":sm", 3).unwrap().text, "sm");
    }

    #[test]
    fn prefix_matches_come_first() {
        let names = ["bob", "alice", "Bobby", "abba"];
        let found = rank("bo", names, |n| vec![*n]);
        assert_eq!(found, vec!["bob", "Bobby"]);
        let found = rank("b", names, |n| vec![*n]);
        assert_eq!(found, vec!["bob", "Bobby", "abba"]);
        // Any of an item's names counts: a nickname or the username.
        let people = [("Nick", "zed"), ("Other", "nobody")];
        let found = rank("ze", people, |(nick, user)| vec![*nick, *user]);
        assert_eq!(found, vec![("Nick", "zed")]);
    }

    #[test]
    fn picks_become_mentions_when_sent() {
        let picks = [
            Completion { shown: "@ann".into(), sent: Some("<@1>".into()) },
            Completion { shown: "@anna".into(), sent: Some("<@2>".into()) },
            Completion { shown: "#general".into(), sent: Some("<#3>".into()) },
            Completion { shown: "@everyone".into(), sent: None },
        ];
        assert_eq!(expand("@anna and @ann in #general", &picks), "<@2> and <@1> in <#3>");
        // A longer word that only starts like a pick is left alone.
        assert_eq!(expand("@annex", &picks), "@annex");
        assert_eq!(expand("@everyone, hi", &picks), "@everyone, hi");
        assert_eq!(expand("ação @ann!", &picks), "ação <@1>!");
    }

    #[test]
    fn the_list_follows_the_word() {
        let mut list = Autocomplete::default();
        assert!(list.update("@a", 2));
        list.set_len(3);
        assert!(list.is_open());
        list.step(-1);
        assert_eq!(list.selected(), 2);
        list.step(1);
        assert_eq!(list.selected(), 0);
        assert!(!list.update("@a", 2));
        assert!(list.dismiss());
        assert!(!list.is_open());
        // Typing on brings it back.
        assert!(list.update("@an", 3));
        list.set_len(1);
        assert!(list.is_open());
        assert!(list.update("@an ", 4));
        assert!(list.query().is_none());
    }
}
