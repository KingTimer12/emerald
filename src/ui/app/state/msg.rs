//! A message as the chat list draws it, resolved once when it arrives.

use gpui::SharedString;

use super::super::super::time;
use crate::discord::model::{Attachment, Embed, Message, Snowflake, User, message_kind};

/// Largest size an attached image is shown at, as in the web client.
const IMAGE_MAX: (f32, f32) = (400., 300.);

pub struct Msg {
    pub id: Snowflake,
    pub author_id: Snowflake,
    pub author: SharedString,
    pub avatar_url: SharedString,
    pub secs: i64,
    pub time: SharedString,
    pub short_time: SharedString,
    pub content: SharedString,
    /// Local calendar day, for day separators.
    pub day: i32,
    /// Sticker names with their image URL (`None` for Lottie).
    pub stickers: Vec<(SharedString, Option<SharedString>)>,
    /// `message_kind`; system kinds render as one muted line.
    pub kind: u8,
    /// Names of the users mentioned, for `<@id>` in the content.
    pub mentions: Vec<(Snowflake, SharedString)>,
    pub mention_roles: Vec<Snowflake>,
    pub mention_everyone: bool,
    pub pinned: bool,
    /// The author changed the text after sending it.
    pub edited: bool,
    pub reply: Option<Reply>,
    /// Image attachments as (thumbnail URL, width, height) in display pixels.
    pub images: Vec<(SharedString, f32, f32)>,
    /// Other attachments, by file name.
    pub files: Vec<SharedString>,
    pub embeds: Vec<Embed>,
    /// Set on our own sends until the server confirms them.
    pub nonce: Option<String>,
    /// Sent but not yet confirmed; drawn dimmed.
    pub pending: bool,
}

/// The message a reply points at; `content` is `None` once it is deleted.
#[derive(Clone)]
pub struct Reply {
    pub author_id: Snowflake,
    pub author: SharedString,
    pub avatar_url: SharedString,
    pub content: Option<SharedString>,
}

impl Reply {
    /// A preview of `msg`, as a reply to it shows it.
    pub fn of(msg: &Msg) -> Self {
        Self {
            author_id: msg.author_id,
            author: msg.author.clone(),
            avatar_url: msg.avatar_url.clone(),
            content: Some(preview(&msg.content).into()),
        }
    }
}

/// The first line of a message, or a stand-in when it has no text.
fn preview(content: &str) -> String {
    content
        .lines()
        .next()
        .map_or_else(|| t!("chat.attachment").to_owned(), str::to_owned)
}

impl Msg {
    /// Our own message as typed, shown until the server echoes it.
    pub fn pending(me: &User, content: String, nonce: String, reply: Option<Reply>) -> Self {
        Self {
            nonce: Some(nonce),
            pending: true,
            reply,
            ..Self::blank(me, Snowflake(0), time::now_secs(), content)
        }
    }

    fn blank(author: &User, id: Snowflake, secs: i64, content: String) -> Self {
        Self {
            id,
            author_id: author.id,
            author: author.display_name().to_owned().into(),
            avatar_url: author.avatar_url().into(),
            secs,
            time: time::header(secs).into(),
            short_time: time::short(secs).into(),
            content: content.into(),
            day: time::day_key(secs),
            stickers: Vec::new(),
            kind: message_kind::DEFAULT,
            mentions: Vec::new(),
            mention_roles: Vec::new(),
            mention_everyone: false,
            pinned: false,
            edited: false,
            reply: None,
            images: Vec::new(),
            files: Vec::new(),
            embeds: Vec::new(),
            nonce: None,
            pending: false,
        }
    }

    pub fn is_pending(&self, nonce: &str) -> bool {
        self.pending && self.nonce.as_deref() == Some(nonce)
    }

    /// Pings `me`, by name, through one of `my_roles`, or with @everyone.
    pub fn mentions_user(&self, me: Snowflake, my_roles: &[Snowflake]) -> bool {
        self.mention_everyone
            || self.mentions.iter().any(|(id, _)| *id == me)
            || self.mention_roles.iter().any(|role| my_roles.contains(role))
    }

    /// Plain text the author can edit here: the message box is one line.
    pub fn editable_text(&self) -> Option<&str> {
        let text = self.content.as_ref();
        let regular = matches!(self.kind, message_kind::DEFAULT | message_kind::REPLY);
        (regular && !self.pending && !text.contains('\n')).then_some(text)
    }
}

/// The names of `users`, for the mentions in a message.
pub fn mention_names(users: &[User]) -> Vec<(Snowflake, SharedString)> {
    users.iter().map(|u| (u.id, u.display_name().to_owned().into())).collect()
}

impl From<Message> for Msg {
    fn from(message: Message) -> Self {
        let mut images = Vec::new();
        let mut files = Vec::new();
        for attachment in &message.attachments {
            match image_attachment(attachment) {
                Some(image) => images.push(image),
                None => files.push(attachment.filename.clone().into()),
            }
        }
        let reply = match message.referenced_message {
            Some(original) => Some(Reply {
                author_id: original.author.id,
                author: original.author.display_name().to_owned().into(),
                avatar_url: original.author.avatar_url().into(),
                // One line, like Discord's reply preview.
                content: Some(preview(&original.content).into()),
            }),
            None if message.kind == message_kind::REPLY => Some(Reply {
                author_id: Snowflake(0),
                author: SharedString::default(),
                avatar_url: SharedString::default(),
                content: None,
            }),
            None => None,
        };
        Self {
            stickers: message
                .sticker_items
                .iter()
                .map(|s| (s.name.clone().into(), s.url().map(Into::into)))
                .collect(),
            kind: message.kind,
            mentions: mention_names(&message.mentions),
            mention_roles: message.mention_roles,
            mention_everyone: message.mention_everyone,
            pinned: message.pinned,
            edited: message.edited_timestamp.is_some(),
            reply,
            images,
            files,
            embeds: message.embeds,
            nonce: message.nonce,
            ..Self::blank(&message.author, message.id, message.id.unix_secs(), message.content)
        }
    }
}

/// Thumbnail URL and display size for an image attachment. The media proxy
/// resizes it, so only the shown pixels are downloaded.
fn image_attachment(attachment: &Attachment) -> Option<(SharedString, f32, f32)> {
    let image = attachment
        .content_type
        .as_deref()
        .is_some_and(|t| t.starts_with("image/"));
    let (Some(proxy), Some(width), Some(height)) =
        (attachment.proxy_url.as_deref(), attachment.width, attachment.height)
    else {
        return None;
    };
    if !image || width == 0 || height == 0 {
        return None;
    }
    let scale = (IMAGE_MAX.0 / width as f32).min(IMAGE_MAX.1 / height as f32).min(1.);
    let (w, h) = ((width as f32 * scale).round(), (height as f32 * scale).round());
    // 2x for Retina, capped by the original size.
    let (fetch_w, fetch_h) = ((w * 2.).min(width as f32) as u32, (h * 2.).min(height as f32) as u32);
    let separator = if proxy.contains('?') { '&' } else { '?' };
    let url = format!("{proxy}{separator}width={fetch_w}&height={fetch_h}");
    Some((url.into(), w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn images_fit_the_frame_and_fetch_at_2x() {
        let attachment = Attachment {
            filename: "a.png".into(),
            proxy_url: Some("https://media/a.png?ex=1".into()),
            content_type: Some("image/png".into()),
            width: Some(1600),
            height: Some(600),
        };
        let (url, w, h) = image_attachment(&attachment).unwrap();
        assert_eq!((w, h), (400., 150.));
        assert_eq!(url.as_ref(), "https://media/a.png?ex=1&width=800&height=300");
    }

    #[test]
    fn non_images_are_files() {
        let attachment = Attachment {
            filename: "a.zip".into(),
            proxy_url: Some("https://media/a.zip".into()),
            content_type: Some("application/zip".into()),
            width: None,
            height: None,
        };
        assert!(image_attachment(&attachment).is_none());
    }
}
