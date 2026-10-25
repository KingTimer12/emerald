//! Polls on a message, and votes on them.

use serde::Deserialize;

use crate::model::{Emoji, Snowflake};

/// The question or an answer of a poll.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct PollMedia {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub emoji: Option<Emoji>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PollAnswer {
    pub answer_id: u32,
    #[serde(default)]
    pub poll_media: PollMedia,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PollAnswerCount {
    pub id: u32,
    #[serde(default)]
    pub count: u32,
    #[serde(default)]
    pub me_voted: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct PollResults {
    /// The poll ended and the counts are final.
    #[serde(default)]
    pub is_finalized: bool,
    #[serde(default)]
    pub answer_counts: Vec<PollAnswerCount>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Poll {
    #[serde(default)]
    pub question: PollMedia,
    #[serde(default)]
    pub answers: Vec<PollAnswer>,
    /// When voting closes, ISO 8601.
    #[serde(default)]
    pub expiry: Option<String>,
    #[serde(default)]
    pub allow_multiselect: bool,
    #[serde(default)]
    pub results: Option<PollResults>,
}

/// MESSAGE_POLL_VOTE_ADD and MESSAGE_POLL_VOTE_REMOVE.
#[derive(Debug, Deserialize)]
pub struct PollVote {
    pub user_id: Snowflake,
    pub channel_id: Snowflake,
    pub message_id: Snowflake,
    pub answer_id: u32,
}
