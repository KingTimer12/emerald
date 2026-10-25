//! Message bodies: sending (with replies, files, stickers and polls),
//! editing, read receipts and poll votes.

use serde_json::{Value, json};

use super::upload::Uploaded;
use crate::model::Snowflake;
use crate::protocol::{NewPoll, ReplyTo};

/// A message to send: its text, what it carries, and what it answers.
/// `nonce` is left out of a forum post's first message, as the web client
/// does.
#[derive(Clone, Copy, Default)]
pub struct Outgoing<'a> {
    pub content: &'a str,
    pub nonce: &'a str,
    pub reply: Option<ReplyTo>,
    pub files: &'a [Uploaded],
    pub poll: Option<&'a NewPoll>,
    pub stickers: &'a [Snowflake],
}

/// `POST /channels/{id}/messages`.
pub fn send(channel: Snowflake, message: &Outgoing<'_>) -> Value {
    let Outgoing {
        content,
        nonce,
        reply,
        files,
        stickers,
        poll,
    } = *message;
    let mut body = json!({
        "mobile_network_type": "unknown",
        "content": content,
        "nonce": nonce,
        "tts": false,
        "flags": 0,
    });
    if !files.is_empty() {
        body["attachments"] = attachments(files);
    }
    if !stickers.is_empty() {
        let ids: Vec<String> = stickers.iter().map(ToString::to_string).collect();
        body["sticker_ids"] = ids.into();
    }
    if let Some(poll) = poll {
        body["poll"] = self::poll(poll);
    }
    if let Some(reply) = reply {
        body["message_reference"] = json!({
            "channel_id": channel.to_string(),
            "message_id": reply.message.to_string(),
        });
        // Like the web client: other mentions still ping, the author not.
        if !reply.mention {
            body["allowed_mentions"] = json!({
                "parse": ["users", "roles", "everyone"],
                "replied_user": false,
            });
        }
    }
    body
}

/// As the web client's poll form builds it; layout 1 is text answers, the
/// only kind made here.
fn poll(poll: &NewPoll) -> Value {
    let answers: Vec<_> = poll.answers.iter().map(|text| json!({ "poll_media": { "text": text } })).collect();
    json!({
        "question": { "text": poll.question },
        "answers": answers,
        "allow_multiselect": poll.multiple,
        "duration": poll.hours,
        "layout_type": 1,
    })
}

/// The `attachments` of a message naming uploaded files. Ids are the files'
/// places in this list, as the web client sends.
pub fn attachments(files: &[Uploaded]) -> Value {
    files
        .iter()
        .enumerate()
        .map(|(ix, file)| {
            let mut attachment = json!({
                "id": ix.to_string(),
                "filename": file.filename,
                "uploaded_filename": file.uploaded_filename,
            });
            // Both are left out unless set, as the web client does.
            if !file.description.is_empty() {
                attachment["description"] = file.description.clone().into();
            }
            if file.spoiler {
                attachment["is_spoiler"] = true.into();
            }
            attachment
        })
        .collect::<Vec<_>>()
        .into()
}

/// `PATCH /channels/{id}/messages/{id}`.
pub fn edit(content: &str) -> Value {
    json!({ "content": content })
}

/// Read receipt.
pub fn ack() -> Value {
    json!({ "token": null })
}

/// Replaces our vote on a poll, as the web client sends it: the answer ids
/// as strings, an empty list taking the vote back.
pub fn vote(answers: &[u32]) -> Value {
    let ids: Vec<String> = answers.iter().map(ToString::to_string).collect();
    json!({ "answer_ids": ids })
}

/// Tells Discord which GIF was picked for `query`.
pub fn select_gif(id: &str, query: &str) -> Value {
    json!({ "id": id, "q": query })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attachments_name_only_what_is_set() {
        let file = |name: &str, description: &str, spoiler| Uploaded {
            filename: name.into(),
            uploaded_filename: format!("up/{name}"),
            description: description.into(),
            spoiler,
        };
        let sent = attachments(&[file("a.png", "", false), file("b.png", "a cat", true)]);
        assert_eq!(
            sent,
            json!([
                { "id": "0", "filename": "a.png", "uploaded_filename": "up/a.png" },
                {
                    "id": "1",
                    "filename": "b.png",
                    "uploaded_filename": "up/b.png",
                    "description": "a cat",
                    "is_spoiler": true,
                },
            ])
        );
    }

    #[test]
    fn polls_ride_on_a_plain_message() {
        let poll = NewPoll {
            question: "Lunch?".into(),
            answers: vec!["Pizza".into(), "Sushi".into()],
            multiple: false,
            hours: 24,
        };
        let body = send(Snowflake(1), &Outgoing { nonce: "9", poll: Some(&poll), ..Outgoing::default() });
        assert_eq!(body["content"], "");
        assert_eq!(body["poll"]["answers"][1]["poll_media"]["text"], "Sushi");
        assert_eq!(body["poll"]["duration"], 24);
        assert!(body.get("message_reference").is_none());
    }
}
