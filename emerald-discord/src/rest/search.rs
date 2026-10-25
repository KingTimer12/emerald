//! Finding messages again: search, recent mentions and pins.

use anyhow::Result;

use super::Rest;
use crate::model::{Message, Snowflake};
use crate::packets::search::{self, PinsAnswer, SearchAnswer};
use crate::protocol::{PinPage, SearchPage, SearchQuery, SearchScope};
use emerald_http::{Call, Policy};

impl Rest {
    /// A page of 25 results, newest first. Discord answers 202 with no
    /// messages while it indexes a server, which comes back as `indexing`.
    pub async fn search(&self, scope: SearchScope, query: &SearchQuery, offset: usize) -> Result<SearchPage> {
        let query = search::messages(query, offset);
        let call = match scope {
            SearchScope::Guild(guild) => {
                Call::get("/guilds/{id}/messages/search", format!("/guilds/{guild}/messages/search?{query}")).major(guild)
            }
            SearchScope::Channel(channel) => {
                Call::get("/channels/{id}/messages/search", format!("/channels/{channel}/messages/search?{query}"))
                    .major(channel)
            }
        };
        let answer: SearchAnswer = self.api.fetch(call).await?;
        Ok(SearchPage {
            indexing: answer.retry_after.is_some() && answer.messages.is_empty(),
            total: answer.total_results,
            messages: answer.messages.into_iter().filter_map(|hit| hit.into_iter().next()).collect(),
        })
    }

    /// Messages that mention the user, across every server and DM.
    pub async fn mentions(&self, before: Option<Snowflake>) -> Result<Vec<Message>> {
        let path = format!("/users/@me/mentions?{}", search::mentions(before));
        self.api.fetch(Call::get("/users/@me/mentions", path).cached(Policy::MICRO)).await
    }

    /// A channel's pins, latest pin first.
    pub async fn pins(&self, channel: Snowflake, before: Option<&str>) -> Result<PinPage> {
        let path = format!("/channels/{channel}/messages/pins?{}", search::pins(before));
        let answer: PinsAnswer = self
            .api
            .fetch(Call::get("/channels/{id}/messages/pins", path).major(channel).cached(Policy::MICRO))
            .await?;
        Ok(PinPage {
            pins: answer.items.into_iter().map(|item| (item.message, item.pinned_at)).collect(),
            has_more: answer.has_more,
        })
    }
}
