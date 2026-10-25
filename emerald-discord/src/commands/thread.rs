//! Threads and forum posts.

use super::captcha::with_captcha;
use super::outcome::failure;
use crate::model::{ChannelKind, Snowflake};
use crate::protocol::{Action, NetEvent, ThreadPage, ThreadQuery, Upload};
use crate::rest::{Outgoing, Rest, ThreadSearch};

/// First messages of these posts; those that fail come back without one.
pub async fn forum_posts(rest: &Rest, forum: Snowflake, threads: Vec<Snowflake>) -> NetEvent {
    let first = rest
        .forum_posts(forum, &threads)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!("forum posts of {forum}: {e:#}");
            Vec::new()
        });
    NetEvent::ForumPosts { forum, first }
}

pub async fn search(rest: &Rest, channel: Snowflake, query: ThreadQuery, offset: usize) -> NetEvent {
    let page = rest
        .thread_search(channel, &query, offset)
        .await
        .map_err(|e| tracing::warn!("threads of {channel}: {e:#}"))
        .ok()
        .map(thread_page);
    NetEvent::ThreadSearch {
        channel,
        query,
        offset,
        page,
    }
}

/// A search page with our memberships folded into the threads they belong to.
fn thread_page(mut search: ThreadSearch) -> ThreadPage {
    for thread in &mut search.threads {
        if let Some(member) = search.members.iter().find(|m| m.id == thread.id) {
            thread.member = Some(member.member());
        }
    }
    ThreadPage {
        threads: search.threads,
        first: search.first_messages,
        has_more: search.has_more,
    }
}

/// A thread in `parent`, from `message` when given.
pub async fn create(
    rest: &Rest,
    parent: Snowflake,
    message: Option<Snowflake>,
    name: String,
    kind: ChannelKind,
    auto_archive: u32,
) -> NetEvent {
    match with_captcha!(|captcha| rest.create_thread(parent, message, &name, kind, auto_archive, captcha).await) {
        Ok(thread) => NetEvent::ThreadCreated(thread),
        Err(e) => failure(Action::CreateThread, e),
    }
}

/// A forum post. As with messages, files go up once and a captcha retry
/// only resends the post.
pub async fn create_post(
    rest: &Rest,
    forum: Snowflake,
    name: String,
    tags: Vec<Snowflake>,
    content: String,
    auto_archive: u32,
    files: Vec<Upload>,
) -> NetEvent {
    let uploaded = match rest.upload_files(forum, &files).await {
        Ok(uploaded) => uploaded,
        Err(e) => return failure(Action::CreateThread, e),
    };
    let first = Outgoing {
        content: &content,
        files: &uploaded,
        ..Outgoing::default()
    };
    match with_captcha!(|captcha| rest.create_post(forum, &name, &tags, auto_archive, &first, captcha).await) {
        Ok(post) => NetEvent::ThreadCreated(post),
        Err(e) => failure(Action::CreateThread, e),
    }
}

pub async fn membership(rest: &Rest, thread: Snowflake, join: bool, forum_post: bool) -> NetEvent {
    match rest.thread_membership(thread, join, forum_post).await {
        Ok(()) => NetEvent::ThreadMembership { thread, joined: join },
        Err(e) => failure(Action::ThreadMembership, e),
    }
}
