//! Message search from the header box: the server's messages, or the DM's,
//! with Discord's filters read from the text (see
//! [`state::search`](super::super::state::search)).

use super::*;

use super::super::state::search::{self, Bad};
use super::super::state::Row;
use emerald_discord::{Command, SearchPage, SearchQuery, SearchScope};

pub(in super::super) struct SearchPanel {
    scope: SearchScope,
    query: SearchQuery,
    /// What was typed, for the title.
    text: SharedString,
    total: u32,
    results: Results,
}

impl SearchPanel {
    /// Whether moving to `channel` (in `guild`) keeps these results in view.
    pub(super) fn covers(&self, channel: Snowflake, guild: Option<Snowflake>) -> bool {
        match self.scope {
            SearchScope::Guild(searched) => guild == Some(searched),
            SearchScope::Channel(searched) => searched == channel,
        }
    }
}

impl Emerald {
    /// Enter in the search box.
    pub(in super::super) fn submit_search(&mut self, text: String, cx: &mut Context<Self>) {
        let Some(channel) = self.selected_channel else { return };
        let space = self.spaces.current();
        let scope = if space.is_dms() { SearchScope::Channel(channel) } else { SearchScope::Guild(space.id) };
        let (query, error) = match self.search_query(&text) {
            Ok(query) if query.is_empty() => return,
            Ok(query) => (query, None),
            Err(error) => (SearchQuery::default(), Some(error)),
        };
        if error.is_none() {
            self.net.post(Command::Search {
                scope,
                query: query.clone(),
                offset: 0,
            });
        }
        self.panel = Some(Panel::Search(SearchPanel {
            scope,
            query,
            text: text.into(),
            total: 0,
            results: Results {
                loading: error.is_none(),
                error,
                ..Results::default()
            },
        }));
        cx.notify();
    }

    /// The typed text as a query, its names found among the people and
    /// channels this client knows.
    fn search_query(&self, text: &str) -> Result<SearchQuery, SharedString> {
        let filters = search::parse(text).map_err(|Bad(word)| SharedString::from(t!("search.bad_filter", filter = word)))?;
        let user = |name: &String| {
            self.find_user(name)
                .ok_or_else(|| SharedString::from(t!("search.unknown_user", name = name)))
        };
        let channel = |name: &String| {
            let name = name.to_lowercase();
            self.spaces
                .current()
                .rows()
                .iter()
                .find_map(|row| match row {
                    Row::Channel(row) if row.name.to_lowercase() == name => Some(row.id),
                    _ => None,
                })
                .ok_or_else(|| SharedString::from(t!("search.unknown_channel", name = name)))
        };
        Ok(SearchQuery {
            content: filters.content,
            authors: filters.from.iter().map(user).collect::<Result<_, _>>()?,
            mentions: filters.mentions.iter().map(user).collect::<Result<_, _>>()?,
            channels: filters.channels.iter().map(channel).collect::<Result<_, _>>()?,
            has: filters.has,
            min_id: filters.min_id,
            max_id: filters.max_id,
            pinned: filters.pinned,
        })
    }

    /// A user by name: a member of the open server by nickname first, then
    /// anyone known by username or display name.
    fn find_user(&self, name: &str) -> Option<Snowflake> {
        let lower = name.to_lowercase();
        let member = self.spaces.current().info().and_then(|guild| {
            guild.members().find_map(|(id, member)| {
                let named = member.nick.as_ref().is_some_and(|n| n.to_lowercase() == lower)
                    || member.user.as_ref().is_some_and(|u| u.username.to_lowercase() == lower);
                named.then_some(id)
            })
        });
        member.or_else(|| self.people.find(name))
    }

    fn more_results(&mut self, cx: &mut Context<Self>) {
        let Some(Panel::Search(search)) = &mut self.panel else { return };
        if search.results.loading {
            return;
        }
        search.results.loading = true;
        self.net.post(Command::Search {
            scope: search.scope,
            query: search.query.clone(),
            offset: search.results.items.len(),
        });
        cx.notify();
    }

    pub(in super::super) fn on_search_results(
        &mut self,
        scope: SearchScope,
        query: SearchQuery,
        offset: usize,
        page: Result<SearchPage, String>,
    ) {
        let Some(Panel::Search(search)) = &mut self.panel else { return };
        if search.scope != scope || search.query != query || search.results.items.len() != offset {
            return;
        }
        let results = &mut search.results;
        results.loading = false;
        match page {
            Ok(page) if page.indexing => results.error = Some(t!("search.indexing").into()),
            Ok(page) => {
                search.total = page.total;
                let spaces = &self.spaces;
                results.items.extend(page.messages.into_iter().map(|message| {
                    let mut found = Found::new(message, spaces);
                    if let SearchScope::Guild(guild) = scope {
                        found.guild = Some(guild);
                    }
                    found
                }));
                results.more = (results.items.len() as u32) < search.total;
            }
            Err(error) => results.error = Some(error.into()),
        }
    }

    pub(super) fn search_title(&self, search: &SearchPanel) -> SharedString {
        if search.results.loading || search.results.error.is_some() {
            return search.text.clone();
        }
        t!("search.results", count = search.total).into()
    }

    pub(super) fn render_search(&self, search: &SearchPanel, cx: &mut Context<Self>) -> AnyElement {
        let in_guild = matches!(search.scope, SearchScope::Guild(_));
        self.render_results("search", &search.results, t!("search.empty"), in_guild, Self::more_results, cx)
    }
}
