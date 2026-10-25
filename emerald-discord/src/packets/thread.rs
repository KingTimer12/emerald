//! Threads and forum posts: creating them, and the answers of the forum
//! endpoints.

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::{Value, json};

use super::message::{Outgoing, attachments};
use crate::model::{Channel, ChannelKind, Message, Snowflake, ThreadMemberEntry};

/// As the web client's "Create Thread" sends it.
pub fn create(name: &str, kind: ChannelKind, auto_archive: u32) -> Value {
    json!({
        "name": name,
        "type": kind.0,
        "auto_archive_duration": auto_archive,
        "location": "Create Thread Sidebar",
    })
}

/// A forum post: the thread and its first message in one request.
pub fn create_post(name: &str, tags: &[Snowflake], auto_archive: u32, first: &Outgoing<'_>) -> Value {
    let tags: Vec<String> = tags.iter().map(ToString::to_string).collect();
    let mut message = json!({ "content": first.content });
    if !first.files.is_empty() {
        message["attachments"] = attachments(first.files);
    }
    json!({
        "name": name,
        "auto_archive_duration": auto_archive,
        "applied_tags": tags,
        "message": message,
    })
}

/// `POST /channels/{id}/post-data` for `threads`.
pub fn post_data(threads: &[Snowflake]) -> Value {
    let ids: Vec<String> = threads.iter().map(ToString::to_string).collect();
    json!({ "thread_ids": ids })
}

/// The answer to `post-data`: each post's first message, by thread id.
#[derive(Deserialize)]
pub struct PostData {
    #[serde(default)]
    pub threads: HashMap<String, Post>,
}

#[derive(Deserialize)]
pub struct Post {
    #[serde(default)]
    pub first_message: Option<Message>,
}

/// A page of a forum's archived posts.
#[derive(Deserialize)]
pub struct ThreadSearch {
    #[serde(default)]
    pub threads: Vec<Channel>,
    /// Our memberships among `threads`.
    #[serde(default)]
    pub members: Vec<ThreadMemberEntry>,
    #[serde(default)]
    pub first_messages: Vec<Message>,
    #[serde(default)]
    pub has_more: bool,
}
