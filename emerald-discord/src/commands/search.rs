//! Search, recent mentions and pins: answers the UI shows in a panel.

use crate::model::Snowflake;
use crate::protocol::{NetEvent, SearchQuery, SearchScope};
use crate::rest::{self, Rest};

/// Discord rejected the token: `AuthExpired` goes instead of the answer.
struct Expired;

/// The answer, or its error worded for the panel.
fn worded<T>(result: anyhow::Result<T>) -> Result<Result<T, String>, Expired> {
    match result {
        Ok(value) => Ok(Ok(value)),
        Err(e) if e.is::<rest::Unauthorized>() => Err(Expired),
        Err(e) => Ok(Err(rest::describe(&e))),
    }
}

pub async fn messages(rest: &Rest, scope: SearchScope, query: SearchQuery, offset: usize) -> NetEvent {
    match worded(rest.search(scope, &query, offset).await) {
        Ok(page) => NetEvent::SearchResults {
            scope,
            query,
            offset,
            page,
        },
        Err(Expired) => NetEvent::AuthExpired,
    }
}

pub async fn mentions(rest: &Rest, before: Option<Snowflake>) -> NetEvent {
    match worded(rest.mentions(before).await) {
        Ok(messages) => NetEvent::Mentions { before, messages },
        Err(Expired) => NetEvent::AuthExpired,
    }
}

/// Unread messages of one channel. Fetched as history is, a page after
/// the last one read; the web client's own inbox request was not found in
/// its bundle, so this is a guess.
pub async fn unread(rest: &Rest, channel: Snowflake, after: Option<Snowflake>) -> NetEvent {
    let page = after.map_or(crate::protocol::Page::Latest, crate::protocol::Page::After);
    match worded(rest.messages(channel, page).await) {
        Ok(messages) => NetEvent::Unread { channel, messages },
        Err(Expired) => NetEvent::AuthExpired,
    }
}

pub async fn pins(rest: &Rest, channel: Snowflake, before: Option<String>) -> NetEvent {
    let result = rest.pins(channel, before.as_deref()).await;
    match worded(result) {
        Ok(page) => NetEvent::Pins { channel, before, page },
        Err(Expired) => NetEvent::AuthExpired,
    }
}
