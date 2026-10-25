//! What a message shows besides text: pictures, file cards and forwarded copies.

use super::*;

/// An image attachment, sized for display.
#[derive(Clone, Debug)]
pub struct Picture {
    /// Thumbnail at twice the shown size.
    pub url: SharedString,
    pub width: f32,
    pub height: f32,
    pub spoiler: bool,
    /// Alt text.
    pub alt: Option<SharedString>,
    /// A video this is the still of, opened on click.
    pub video: Option<SharedString>,
    /// The whole animated GIF, played instead of the still while the
    /// window has focus.
    pub animated: Option<SharedString>,
}

/// An attachment drawn as a card: its name, size and what it is.
#[derive(Clone, Debug)]
pub struct FileCard {
    pub name: SharedString,
    /// The file on the CDN, opened in the browser.
    pub url: SharedString,
    pub size: u64,
    pub kind: FileKind,
    pub spoiler: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FileKind {
    Other,
    Video,
    Audio,
    /// A voice message, this many seconds long.
    Voice(f64),
}

/// A forwarded message: what it said and where it came from.
pub struct Forward {
    pub content: SharedString,
    pub mentions: Vec<(Snowflake, SharedString)>,
    pub images: Vec<Picture>,
    pub files: Vec<FileCard>,
    pub embeds: Vec<Embed>,
    pub stickers: Vec<Sticker>,
    pub guild: Option<Snowflake>,
    pub channel: Option<Snowflake>,
    pub message: Option<Snowflake>,
    /// When the original was sent.
    pub secs: Option<i64>,
}

/// Attachments split into those drawn as pictures and those drawn as
/// cards. A voice message's audio file is a voice card.
pub(super) fn sort_attachments(attachments: &[Attachment], voice: bool) -> (Vec<Picture>, Vec<FileCard>) {
    let mut images = Vec::new();
    let mut files = Vec::new();
    for attachment in attachments {
        match image_attachment(attachment) {
            Some(image) => images.push(image),
            None => files.push(file_card(attachment, voice)),
        }
    }
    (images, files)
}

pub(super) fn file_card(attachment: &Attachment, voice: bool) -> FileCard {
    let content_type = attachment.content_type.as_deref().unwrap_or_default();
    let kind = if content_type.starts_with("audio/") {
        match attachment.duration_secs {
            Some(secs) if voice => FileKind::Voice(secs),
            _ => FileKind::Audio,
        }
    } else if content_type.starts_with("video/") {
        FileKind::Video
    } else {
        FileKind::Other
    };
    FileCard {
        name: attachment.filename.clone().into(),
        url: attachment.url.clone().into(),
        size: attachment.size,
        kind,
        spoiler: attachment.spoiler(),
    }
}

/// Thumbnail URL and display size for an image attachment. The media proxy
/// resizes it, so only the shown pixels are downloaded.
/// Videos draw as their still, which the media proxy makes from the first
/// frame when asked for WebP, as the web client asks for its posters.
pub(super) fn image_attachment(attachment: &Attachment) -> Option<Picture> {
    let content_type = attachment.content_type.as_deref().unwrap_or_default();
    let video = content_type.starts_with("video/");
    if !content_type.starts_with("image/") && !video {
        return None;
    }
    let (Some(proxy), Some(width), Some(height)) =
        (attachment.proxy_url.as_deref(), attachment.width, attachment.height)
    else {
        return None;
    };
    let mut picture = sized_picture(proxy, width, height, video.then_some("format=webp"))?;
    picture.spoiler = attachment.spoiler();
    picture.alt = attachment.description.clone().filter(|d| !d.trim().is_empty()).map(Into::into);
    picture.video = video.then(|| attachment.url.clone().into());
    // The proxy's copy at full size keeps every frame.
    picture.animated = (content_type == "image/gif").then(|| proxy.to_owned().into());
    Some(picture)
}

/// The image at `proxy`, a media proxy URL, fit into the frame Discord
/// draws images in and fetched at twice that for Retina; `extra` goes in
/// the query too.
pub fn sized_picture(proxy: &str, width: u32, height: u32, extra: Option<&str>) -> Option<Picture> {
    if width == 0 || height == 0 {
        return None;
    }
    let scale = (IMAGE_MAX.0 / width as f32).min(IMAGE_MAX.1 / height as f32).min(1.);
    let (w, h) = ((width as f32 * scale).round().max(1.), (height as f32 * scale).round().max(1.));
    // 2x for Retina, capped by the original size.
    let (fetch_w, fetch_h) = ((w * 2.).min(width as f32) as u32, (h * 2.).min(height as f32) as u32);
    let separator = if proxy.contains('?') { '&' } else { '?' };
    let extra = extra.map(|e| format!("{e}&")).unwrap_or_default();
    let url = format!("{proxy}{separator}{extra}width={fetch_w}&height={fetch_h}");
    Some(Picture {
        url: url.into(),
        width: w,
        height: h,
        spoiler: false,
        alt: None,
        video: None,
        animated: None,
    })
}
