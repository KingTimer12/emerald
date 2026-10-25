//! History, sending and what can be done to a message.

use anyhow::{Result, bail};
use reqwest::header::CONTENT_TYPE;

use super::encoding::{encode_component, encode_path};
use super::{DiscordCall, HISTORY_PAGE, Rest};
use crate::model::{Emoji, Message, Snowflake};
use crate::packets::context;
use crate::packets::message::{self, Outgoing};
use crate::packets::upload::{self, Uploaded, UploadUrls};
use crate::protocol::{Page, ReactionSource, Upload};
use emerald_http::{Call, Policy, Solution};

impl Rest {
    /// A page of history, oldest first. The query keeps the web client's
    /// key order: `before`, `after`, `limit`, `around`.
    pub async fn messages(&self, channel: Snowflake, page: Page) -> Result<Vec<Message>> {
        let path = match page {
            Page::Latest => format!("/channels/{channel}/messages?limit={HISTORY_PAGE}"),
            Page::Before(id) => format!("/channels/{channel}/messages?before={id}&limit={HISTORY_PAGE}"),
            Page::After(id) => format!("/channels/{channel}/messages?after={id}&limit={HISTORY_PAGE}"),
            Page::Around(id) => format!("/channels/{channel}/messages?limit={HISTORY_PAGE}&around={id}"),
        };
        let mut messages: Vec<Message> = self
            .api
            .fetch(Call::get("/channels/{id}/messages", path).major(channel).cached(Policy::MICRO))
            .await?;
        // Newest first for most anchors; ids sort them whatever the order.
        messages.sort_by_key(|m| m.id);
        Ok(messages)
    }

    /// `nonce` comes back on the MESSAGE_CREATE echo, which is how the UI
    /// swaps its pending copy for the real message.
    pub async fn send_message(
        &self,
        channel: Snowflake,
        outgoing: &Outgoing<'_>,
        captcha: Option<&Solution>,
    ) -> Result<Message> {
        let call = Call::post("/channels/{id}/messages", format!("/channels/{channel}/messages"))
            .major(channel)
            .json(message::send(channel, outgoing))
            .captcha(captcha);
        // The poll form sends with `location: poll_creation`, which the
        // web client's HTTP layer turns into this header; inferred from the
        // bundle, not seen on the wire.
        let call = if outgoing.poll.is_some() { call.context(context::location("poll_creation")) } else { call };
        self.api.fetch(call).await
    }

    /// Puts each file in Discord's storage, as the web client does: ask the
    /// channel for an upload URL, then PUT the bytes there. The message then
    /// names the files by their `uploaded_filename`.
    pub async fn upload_files(&self, channel: Snowflake, files: &[Upload]) -> Result<Vec<Uploaded>> {
        let mut uploaded = Vec::with_capacity(files.len());
        for file in files {
            let answer: UploadUrls = self
                .api
                .fetch(
                    Call::post("/channels/{id}/attachments", format!("/channels/{channel}/attachments"))
                        .major(channel)
                        .json(upload::request(file)),
                )
                .await?;
            let Some(target) = answer.attachments.into_iter().next() else {
                bail!("Discord gave no upload URL for {}", file.name);
            };
            if !target.upload_url.starts_with("https://") {
                bail!("refusing a non-HTTPS upload URL for {}", file.name);
            }
            let response = self
                .uploads
                .put(&target.upload_url)
                .header(CONTENT_TYPE, "application/octet-stream")
                .body(reqwest::Body::from(bytes::Bytes::from_owner(file.bytes.clone())))
                .send()
                .await?;
            if !response.status().is_success() {
                bail!("uploading {} failed: {}", file.name, response.status());
            }
            uploaded.push(Uploaded {
                filename: file.name.clone(),
                uploaded_filename: target.upload_filename,
                description: file.description.clone(),
                spoiler: file.spoiler,
            });
        }
        Ok(uploaded)
    }

    pub async fn edit_message(&self, channel: Snowflake, message: Snowflake, content: &str) -> Result<()> {
        self.api.run(
            Call::patch("/channels/{id}/messages/{id}", format!("/channels/{channel}/messages/{message}"))
                .major(channel)
                .json(message::edit(content)),
        )
        .await
    }

    pub async fn delete_message(&self, channel: Snowflake, message: Snowflake) -> Result<()> {
        self.api.run(
            Call::delete("/channels/{id}/messages/{id}", format!("/channels/{channel}/messages/{message}"))
                .major(channel),
        )
        .await
    }

    /// Replaces our vote on a poll, as the web client sends it: the answer
    /// ids as strings, an empty list taking the vote back.
    pub async fn vote_poll(&self, channel: Snowflake, message: Snowflake, answers: &[u32]) -> Result<()> {
        let path = format!("/channels/{channel}/polls/{message}/answers/@me");
        self.api.run(
            Call::put("/channels/{id}/polls/{id}/answers/@me", path)
                .major(channel)
                .json(message::vote(answers)),
        )
        .await
    }

    pub async fn pin_message(&self, channel: Snowflake, message: Snowflake, pin: bool) -> Result<()> {
        let path = format!("/channels/{channel}/messages/pins/{message}");
        let call = if pin {
            Call::put("/channels/{id}/messages/pins/{id}", path)
        } else {
            Call::delete("/channels/{id}/messages/pins/{id}", path)
        };
        self.api.run(call.major(channel)).await
    }

    /// Our normal reaction, as the web client adds it: `type=0` and the
    /// surface it came from as `location`.
    pub async fn add_reaction(
        &self,
        channel: Snowflake,
        message: Snowflake,
        emoji: &Emoji,
        source: ReactionSource,
    ) -> Result<()> {
        let path = format!(
            "/channels/{channel}/messages/{message}/reactions/{}/@me?location={}&type=0",
            encode_path(&emoji.route_key()),
            encode_component(source.location()),
        );
        self.api.run(Call::put("/channels/{id}/messages/{id}/reactions/{emoji}/@me", path).major(channel))
            .await
    }

    /// Removes our normal reaction through the typed route the web client uses.
    pub async fn remove_reaction(
        &self,
        channel: Snowflake,
        message: Snowflake,
        emoji: &Emoji,
        source: ReactionSource,
    ) -> Result<()> {
        let path = format!(
            "/channels/{channel}/messages/{message}/reactions/{}/0/@me?location={}&burst=false",
            encode_path(&emoji.route_key()),
            encode_component(source.location()),
        );
        self.api.run(Call::delete("/channels/{id}/messages/{id}/reactions/{emoji}/{type}/@me", path).major(channel))
            .await
    }

    /// Read receipt, as the web client sends when a channel is on screen.
    pub async fn ack(&self, channel: Snowflake, message: Snowflake) -> Result<()> {
        self.api.run(
            Call::post(
                "/channels/{id}/messages/{id}/ack",
                format!("/channels/{channel}/messages/{message}/ack"),
            )
            .major(channel)
            .json(message::ack()),
        )
        .await
    }

    /// Shows "… is typing" to the other side for about ten seconds.
    pub async fn typing(&self, channel: Snowflake) -> Result<()> {
        self.api.run(Call::post("/channels/{id}/typing", format!("/channels/{channel}/typing")).major(channel))
            .await
    }
}
