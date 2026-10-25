//! History, sending in all its forms, and what can be done to a message.

use super::captcha::with_captcha;
use super::outcome::{or_expired, outcome};
use crate::model::{Emoji, Gif, Snowflake};
use crate::protocol::{Action, NetEvent, NewPoll, Page, ReactionSource, ReplyTo, Upload};
use crate::rest::{self, Outgoing, Rest};

pub async fn history(rest: &Rest, channel: Snowflake, page: Page) -> NetEvent {
    match rest.messages(channel, page).await {
        Ok(messages) => NetEvent::History {
            channel,
            page,
            messages,
        },
        Err(e) => or_expired(e, |e| NetEvent::HistoryFailed {
            channel,
            forbidden: e.is::<rest::Forbidden>(),
            error: rest::describe(&e),
        }),
    }
}

/// What the UI needs back when a send fails: the text and files, to try
/// again.
struct Unsent {
    channel: Snowflake,
    nonce: String,
    content: String,
    files: Vec<Upload>,
}

impl Unsent {
    fn bare(channel: Snowflake, nonce: String) -> Self {
        Self {
            channel,
            nonce,
            content: String::new(),
            files: Vec::new(),
        }
    }

    /// The sent message, or `SendFailed` handing everything back.
    fn answer(self, result: anyhow::Result<crate::model::Message>) -> NetEvent {
        match result {
            // Usually the gateway echo arrives first; the UI drops the duplicate.
            Ok(message) => NetEvent::MessageCreate(Box::new(message)),
            Err(e) => self.failed(e),
        }
    }

    fn failed(self, error: anyhow::Error) -> NetEvent {
        or_expired(error, |e| NetEvent::SendFailed {
            channel: self.channel,
            nonce: self.nonce,
            content: self.content,
            files: self.files,
            error: rest::describe(&e),
        })
    }
}

pub async fn send(
    rest: &Rest,
    channel: Snowflake,
    content: String,
    nonce: String,
    reply: Option<ReplyTo>,
    files: Vec<Upload>,
) -> NetEvent {
    // Files go up once; a captcha retry only resends the message.
    let uploaded = match rest.upload_files(channel, &files).await {
        Ok(uploaded) => uploaded,
        Err(e) => return Unsent { channel, nonce, content, files }.failed(e),
    };
    let message = Outgoing {
        content: &content,
        nonce: &nonce,
        reply,
        files: &uploaded,
        ..Outgoing::default()
    };
    let result = with_captcha!(|captcha| rest.send_message(channel, &message, captcha).await);
    Unsent { channel, nonce, content, files }.answer(result)
}

/// A sticker sent on its own, as picking one in the web client does.
pub async fn send_sticker(rest: &Rest, channel: Snowflake, sticker: Snowflake, nonce: String) -> NetEvent {
    let stickers = [sticker];
    let message = Outgoing {
        nonce: &nonce,
        stickers: &stickers,
        ..Outgoing::default()
    };
    let result = with_captcha!(|captcha| rest.send_message(channel, &message, captcha).await);
    Unsent::bare(channel, nonce).answer(result)
}

pub async fn send_gif(rest: &Rest, channel: Snowflake, gif: Gif, query: String, nonce: String) -> NetEvent {
    // Only bookkeeping on Discord's side; the GIF goes out either way.
    if let Err(e) = rest.select_gif(&gif.id, &query).await {
        tracing::debug!("gif select: {e:#}");
    }
    let message = Outgoing {
        content: &gif.url,
        nonce: &nonce,
        ..Outgoing::default()
    };
    let result = with_captcha!(|captcha| rest.send_message(channel, &message, captcha).await);
    Unsent::bare(channel, nonce).answer(result)
}

/// Posts a poll; the gateway brings the message.
pub async fn send_poll(rest: &Rest, channel: Snowflake, nonce: String, poll: NewPoll) -> NetEvent {
    let message = Outgoing {
        nonce: &nonce,
        poll: Some(&poll),
        ..Outgoing::default()
    };
    outcome(Action::CreatePoll, with_captcha!(|captcha| rest.send_message(channel, &message, captcha).await))
}

pub async fn edit(rest: &Rest, channel: Snowflake, message: Snowflake, content: String) -> NetEvent {
    outcome(Action::Message, rest.edit_message(channel, message, &content).await)
}

pub async fn delete(rest: &Rest, channel: Snowflake, message: Snowflake) -> NetEvent {
    outcome(Action::Message, rest.delete_message(channel, message).await)
}

pub async fn vote(rest: &Rest, channel: Snowflake, message: Snowflake, answers: Vec<u32>) -> NetEvent {
    outcome(Action::Message, rest.vote_poll(channel, message, &answers).await)
}

pub async fn pin(rest: &Rest, channel: Snowflake, message: Snowflake, pin: bool) -> NetEvent {
    outcome(Action::Message, rest.pin_message(channel, message, pin).await)
}

/// The UI already shows the reaction; only a failure, which undoes it,
/// needs an event.
pub async fn react(
    rest: &Rest,
    channel: Snowflake,
    message: Snowflake,
    emoji: Emoji,
    add: bool,
    source: ReactionSource,
) -> Option<NetEvent> {
    let result = if add {
        rest.add_reaction(channel, message, &emoji, source).await
    } else {
        rest.remove_reaction(channel, message, &emoji, source).await
    };
    let error = result.err()?;
    Some(or_expired(error, |e| NetEvent::ReactionFailed {
        channel,
        message,
        emoji,
        add,
        error: rest::describe(&e),
    }))
}

pub async fn ack(rest: &Rest, channel: Snowflake, message: Snowflake) -> Option<NetEvent> {
    if let Err(e) = rest.ack(channel, message).await {
        tracing::debug!("ack {channel}/{message}: {e:#}");
    }
    None
}

pub async fn typing(rest: &Rest, channel: Snowflake) -> Option<NetEvent> {
    if let Err(e) = rest.typing(channel).await {
        tracing::debug!("typing in {channel}: {e:#}");
    }
    None
}
