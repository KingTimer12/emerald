//! Threads and forum posts.

use anyhow::Result;

use super::{DiscordCall, FORUM_PAGE, Rest};
use crate::model::{Channel, ChannelKind, Message, Snowflake};
use crate::packets::message::Outgoing;
use crate::packets::thread::{self, PostData, ThreadSearch};
use emerald_http::{Call, Policy, Solution};

impl Rest {
    /// The first message of each forum post in `threads`, at most 10 at a
    /// time as the web client asks.
    pub async fn forum_posts(&self, forum: Snowflake, threads: &[Snowflake]) -> Result<Vec<(Snowflake, Message)>> {
        let data: PostData = self
            .api
            .fetch(
                Call::post("/channels/{id}/post-data", format!("/channels/{forum}/post-data"))
                    .major(forum)
                    .json(thread::post_data(threads)),
            )
            .await?;
        Ok(data
            .threads
            .into_iter()
            .filter_map(|(id, post)| Some((Snowflake(id.parse().ok()?), post.first_message?)))
            .collect())
    }

    /// A page of a forum's archived posts, newest activity first.
    pub async fn archived_posts(&self, forum: Snowflake, offset: usize) -> Result<ThreadSearch> {
        let path = format!(
            "/channels/{forum}/threads/search?archived=true&sort_by=last_message_time&sort_order=desc\
             &limit={FORUM_PAGE}&tag_setting=match_some&offset={offset}"
        );
        self.api.fetch(Call::get("/channels/{id}/threads/search", path).major(forum).cached(Policy::MICRO))
            .await
    }

    /// A thread in `parent`, started from `message` when given, as the web
    /// client's "Create Thread" sends it.
    pub async fn create_thread(
        &self,
        parent: Snowflake,
        message: Option<Snowflake>,
        name: &str,
        kind: ChannelKind,
        auto_archive: u32,
        captcha: Option<&Solution>,
    ) -> Result<Channel> {
        let call = match message {
            Some(message) => Call::post(
                "/channels/{id}/messages/{id}/threads",
                format!("/channels/{parent}/messages/{message}/threads"),
            ),
            None => Call::post("/channels/{id}/threads", format!("/channels/{parent}/threads")),
        };
        self.api.fetch(
            call.major(parent)
                .json(thread::create(name, kind, auto_archive))
                .captcha(captcha),
        )
        .await
    }

    /// A forum post: the thread and its first message in one request.
    pub async fn create_post(
        &self,
        forum: Snowflake,
        name: &str,
        tags: &[Snowflake],
        auto_archive: u32,
        first: &Outgoing<'_>,
        captcha: Option<&Solution>,
    ) -> Result<Channel> {
        self.api.fetch(
            Call::post("/channels/{id}/threads", format!("/channels/{forum}/threads?use_nested_fields=true"))
                .major(forum)
                .json(thread::create_post(name, tags, auto_archive, first))
                .captcha(captcha),
        )
        .await
    }

    /// Joins or leaves `thread`. The `location` is the surface the web
    /// client's button sits on: "Forum Toolbar" for a forum post, and
    /// "Toolbar Overflow" (the header's menu) for any other thread.
    pub async fn thread_membership(&self, thread: Snowflake, join: bool, forum_post: bool) -> Result<()> {
        let location = if forum_post { "Forum%20Toolbar" } else { "Toolbar%20Overflow" };
        let path = format!("/channels/{thread}/thread-members/@me?location={location}");
        let route = "/channels/{id}/thread-members/@me";
        let call = if join { Call::post(route, path) } else { Call::delete(route, path) };
        self.api.run(call.major(thread)).await
    }
}
