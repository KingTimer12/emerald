//! What a message carries besides its text: pictures and video stills,
//! file cards (voice messages and audio included), embeds with their
//! fields and media, and the box a forwarded message comes in.

use gpui::{AnyElement, Context, ElementId, FontWeight, SharedString, div, img, prelude::*, px, rgb};

use super::super::super::{icons, markdown, theme, time};
use super::super::attachments::format_size;
use super::super::state::{FileCard, FileKind, Forward, Picture, sized_picture};
use super::super::{Emerald, widgets};
use super::list::spoiler_pill;
use crate::discord::model::{Embed, EmbedMedia, Snowflake};

/// Thumbnails beside an embed's text, as the web client sizes them.
const EMBED_THUMBNAIL: f32 = 80.;
/// Embed parts get spoiler scopes in blocks of this many: the description
/// first, then each field.
const EMBED_PARTS: u32 = 32;
/// A forwarded message's own spoilers are kept apart from the forward's.
const FORWARD_PART: u32 = 10_000;

impl Emerald {
    /// An image attachment or a video's still. Spoilers stay covered, and
    /// are not even downloaded, until clicked; alt text shows on hover and
    /// as a badge; videos open in the browser from their play button.
    pub(super) fn render_picture(&self, picture: &Picture, id: ElementId, cx: &mut Context<Self>) -> AnyElement {
        let frame = div()
            .id(id)
            .relative()
            .flex_none()
            .w(px(picture.width))
            .h(px(picture.height))
            .rounded_lg()
            .overflow_hidden();
        if picture.spoiler && !self.revealed.contains(&picture.url) {
            return self.spoiler_cover(frame, picture.url.clone(), cx);
        }
        let animated = picture
            .animated
            .as_ref()
            .and_then(|url| self.animation(url, picture.width, picture.height));
        let shown = match (animated, self.avatar_image(&picture.url)) {
            (Some(frames), _) => frame.child(img(frames).id("animated").size_full()),
            (None, Some(image)) => frame.child(img(image).size_full()),
            (None, None) => frame.bg(theme::hover()),
        };
        let shown = match picture.video.as_ref().and_then(|url| markdown::web_url(url)) {
            Some(url) => {
                let url = url.to_owned();
                shown
                    .cursor_pointer()
                    .child(play_button(56.))
                    .on_click(move |_, _, cx| cx.open_url(&url))
            }
            None => shown,
        };
        match &picture.alt {
            Some(alt) => shown
                .tooltip(widgets::tooltip(alt.clone()))
                .child(badge(t!("chat.alt")))
                .into_any_element(),
            None => shown.into_any_element(),
        }
    }

    /// `frame` covered until clicked, which reveals what `key` names.
    fn spoiler_cover(&self, frame: gpui::Stateful<gpui::Div>, key: SharedString, cx: &mut Context<Self>) -> AnyElement {
        frame
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .bg(rgb(theme::BASE_LOWEST))
            .child(spoiler_pill())
            .on_click(cx.listener(move |this, _, _, cx| {
                this.revealed.insert(key.clone());
                cx.notify();
            }))
            .into_any_element()
    }

    /// An attachment drawn as a card: name and size, with a play button
    /// for audio and voice messages. Clicking opens the file in the
    /// browser; a spoiler is covered until clicked, like an image.
    pub(super) fn render_file(&self, file: &FileCard, id: ElementId, cx: &mut Context<Self>) -> AnyElement {
        let card = div()
            .id(id)
            .relative()
            .max_w(px(432.))
            .flex()
            .items_center()
            .gap_3()
            .p_3()
            .rounded_lg()
            .border_1()
            .border_color(theme::border())
            .bg(rgb(theme::BASE_LOW));
        if file.spoiler && !self.revealed.contains(&file.url) {
            // The card keeps its size, its contents out of sight.
            return self.spoiler_cover(card.h(px(66.)).w(px(432.)), file.url.clone(), cx);
        }
        let url = markdown::web_url(&file.url).map(str::to_owned);
        let (name, detail): (SharedString, SharedString) = match file.kind {
            FileKind::Voice(secs) => (t!("chat.voice_message").into(), clock(secs).into()),
            _ => (file.name.clone(), format_size(file.size)),
        };
        let icon = match file.kind {
            FileKind::Audio | FileKind::Voice(_) | FileKind::Video => play_button(36.).into_any_element(),
            FileKind::Other => icons::icon(icons::FILE, 32., theme::INTERACTIVE).into_any_element(),
        };
        card.child(icon)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(0.))
                    .child(
                        div()
                            .truncate()
                            .text_color(rgb(if url.is_some() { theme::LINK } else { theme::TEXT_STRONG }))
                            .child(name),
                    )
                    .child(div().text_xs().text_color(theme::muted()).child(detail)),
            )
            .when_some(url, |card, url| {
                card.cursor_pointer()
                    .tooltip(widgets::tooltip(t!("chat.open_file")))
                    .on_click(move |_, _, cx| cx.open_url(&url))
            })
            .into_any_element()
    }

    /// An embed: an image or GIF preview on its own, or a card with the
    /// color bar holding provider, author, title, description, fields,
    /// thumbnail and image. Embeds with nothing to show are skipped.
    pub(super) fn render_embed(&self, embed: &Embed, message: Snowflake, prefix: &str, n: usize) -> Option<AnyElement> {
        let id = |part: &str| ElementId::Name(format!("{prefix}-embed-{n}-{part}").into());
        let opens = embed.url.as_deref().and_then(markdown::web_url).map(str::to_owned);
        match embed.kind.as_deref() {
            // A link to an image or a GIF: the media alone.
            Some("image") | Some("gifv") => {
                let media = embed.thumbnail.as_ref().or(embed.image.as_ref())?;
                let gif = embed.kind.as_deref() == Some("gifv");
                return Some(self.render_embed_media(media, gif, opens, id("media")));
            }
            _ => {}
        }
        let provider = embed.provider.as_ref().and_then(|p| p.name.clone());
        let author = embed.author.as_ref().and_then(|a| a.name.clone());
        let footer = embed.footer.as_ref().and_then(|f| f.text.clone());
        let video = embed.kind.as_deref() == Some("video") && embed.video.is_some();
        // A video's thumbnail is its still, drawn large like an image.
        let large = embed.image.as_ref().or(embed.thumbnail.as_ref().filter(|_| video));
        let thumbnail = embed.thumbnail.as_ref().filter(|_| !video && embed.image.is_none());
        let has_text = embed.title.is_some() || embed.description.is_some() || author.is_some() || !embed.fields.is_empty();
        if !has_text && large.is_none() {
            return None;
        }
        let small = |text: String, size: f32, color: u32| div().text_size(px(size)).text_color(rgb(color)).child(text);
        let title = embed.title.clone().map(|title| {
            // Embed URLs come from the sender; open only http(s).
            let element = div()
                .text_size(px(16.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(if opens.is_some() { theme::LINK } else { theme::TEXT_STRONG }))
                .child(title);
            match opens.clone() {
                Some(url) => element
                    .id(id("title"))
                    .cursor_pointer()
                    .on_click(move |_, _, cx| cx.open_url(&url))
                    .into_any_element(),
                None => element.into_any_element(),
            }
        });
        let part = 1 + n as u32 * EMBED_PARTS;
        let description = embed.description.as_ref().map(|description| {
            div()
                .text_size(px(14.))
                .line_height(px(18.))
                .child(self.rich_embed(id("description"), description, &[], message, part))
        });
        let fields = self.render_fields(embed, message, part, &id);
        let thumbnail = thumbnail.and_then(|media| {
            let url = media.proxy_url.as_deref().or(media.url.as_deref())?;
            let separator = if url.contains('?') { '&' } else { '?' };
            let side = (EMBED_THUMBNAIL * 2.) as u32;
            let image = self.avatar_image(&format!("{url}{separator}width={side}&height={side}"))?;
            Some(
                div()
                    .flex_none()
                    .size(px(EMBED_THUMBNAIL))
                    .rounded_md()
                    .overflow_hidden()
                    .child(img(image).size_full().object_fit(gpui::ObjectFit::Cover)),
            )
        });
        let large = large.map(|media| self.render_embed_media(media, false, opens.clone().filter(|_| video), id("image")));
        Some(
            div()
                .max_w(px(520.))
                .flex()
                .rounded_md()
                .overflow_hidden()
                .bg(rgb(theme::BASE_LOW))
                .border_1()
                .border_color(theme::border())
                .child(div().w(px(4.)).flex_none().bg(rgb(embed.color.unwrap_or(theme::INTERACTIVE_MUTED))))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w(px(0.))
                        .gap_2()
                        .px_3()
                        .py_2()
                        .child(
                            div()
                                .flex()
                                .gap_3()
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .flex_1()
                                        .min_w(px(0.))
                                        .gap_1()
                                        .children(provider.map(|p| small(p, 12., theme::TEXT_MUTED)))
                                        .children(author.map(|a| {
                                            small(a, 14., theme::TEXT_STRONG).font_weight(FontWeight::SEMIBOLD)
                                        }))
                                        .children(title)
                                        .children(description)
                                        .children(fields),
                                )
                                .children(thumbnail),
                        )
                        .children(large)
                        .children(footer.map(|f| small(f, 12., theme::TEXT_MUTED))),
                )
                .into_any_element(),
        )
    }

    /// An embed's fields: inline ones up to three to a row, as Discord
    /// lays them out, the others on rows of their own.
    fn render_fields(
        &self,
        embed: &Embed,
        message: Snowflake,
        part: u32,
        id: &dyn Fn(&str) -> ElementId,
    ) -> Option<AnyElement> {
        if embed.fields.is_empty() {
            return None;
        }
        let mut rows: Vec<Vec<AnyElement>> = Vec::new();
        let mut inline_run = 0;
        for (ix, field) in embed.fields.iter().enumerate().take(EMBED_PARTS as usize - 1) {
            let cell = div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w(px(0.))
                .text_size(px(14.))
                .line_height(px(18.))
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(theme::TEXT_STRONG))
                        .child(self.rich_line(id(&format!("field-name-{ix}")), &field.name, &[])),
                )
                .child(self.rich_embed(id(&format!("field-{ix}")), &field.value, &[], message, part + 1 + ix as u32))
                .into_any_element();
            if field.inline && inline_run > 0 && inline_run < 3 {
                inline_run += 1;
                rows.last_mut().expect("an inline run has a row").push(cell);
            } else {
                inline_run = usize::from(field.inline);
                rows.push(vec![cell]);
            }
        }
        Some(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .mt_1()
                .children(rows.into_iter().map(|row| div().flex().gap_2().children(row)))
                .into_any_element(),
        )
    }

    /// An embed's image, thumbnail-as-image or GIF, sized like an
    /// attachment and opening `opens` on click. A GIF embed, or an image
    /// link to a `.gif`, plays its original (a GIF, or the animated WebP
    /// Klipy serves) while the window has focus; otherwise the still.
    fn render_embed_media(&self, media: &EmbedMedia, gif: bool, opens: Option<String>, id: ElementId) -> AnyElement {
        let source = media.proxy_url.as_deref().or(media.url.as_deref()).unwrap_or_default();
        let (width, height) = (media.width.unwrap_or(400), media.height.unwrap_or(300));
        let Some(picture) = sized_picture(source, width, height, None) else {
            return div().into_any_element();
        };
        let original = media.url.as_deref().and_then(markdown::web_url);
        let moving = gif || original.is_some_and(|url| url.split(['?', '#']).next().is_some_and(|p| p.ends_with(".gif")));
        let animated = original
            .filter(|_| moving)
            .and_then(|url| self.animation(url, picture.width, picture.height));
        let frame = div()
            .id(id.clone())
            .relative()
            .flex_none()
            .w(px(picture.width))
            .h(px(picture.height))
            .rounded_lg()
            .overflow_hidden();
        let frame = match animated {
            // An id lets gpui keep the animation's frame between redraws.
            Some(frames) => frame.child(img(frames).id(ElementId::NamedChild(Box::new(id), "gif".into())).size_full()),
            None => match self.avatar_image(&picture.url) {
                Some(image) => frame.child(img(image).size_full()),
                None => frame.bg(theme::hover()),
            },
        };
        let frame = frame.when(gif, |frame| frame.child(badge(t!("chat.gif"))));
        match opens {
            Some(url) => frame
                .cursor_pointer()
                .when(!gif, |frame| frame.child(play_button(56.)))
                .on_click(move |_, _, cx| cx.open_url(&url))
                .into_any_element(),
            None => frame.into_any_element(),
        }
    }

    /// The frames of the animation at `url`, decoded at twice the shown
    /// size for Retina; `None` while the window is in the background, so
    /// nothing plays there, and until they arrive.
    fn animation(&self, url: &str, width: f32, height: f32) -> Option<std::sync::Arc<gpui::RenderImage>> {
        if !self.window_active {
            return None;
        }
        self.images.animated(url, (width * 2.) as u32, (height * 2.) as u32, &*self.net)
    }

    /// A forwarded message: the label, the original's text and media
    /// behind a bar, and where it came from, which opens it.
    pub(super) fn render_forward(&self, forward: &Forward, message: Snowflake, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let prefix = format!("forward-{ix}");
        let origin = forward
            .channel
            .and_then(|channel| self.spaces.channel(channel))
            .and_then(|channel| channel.name.clone())
            .map(|name| format!("#{name}"));
        let when = forward.secs.map(time::header);
        let footer = [origin, when].into_iter().flatten().collect::<Vec<_>>().join(" • ");
        let target = forward.channel.map(|channel| (forward.guild, channel, forward.message));
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .text_size(px(14.))
                    .italic()
                    .text_color(theme::muted())
                    .child(icons::icon(icons::FORWARD, 16., theme::TEXT_MUTED))
                    .child(t!("chat.forwarded")),
            )
            .child(
                div()
                    .flex()
                    .child(div().flex_none().w(px(4.)).rounded_sm().bg(rgb(theme::INTERACTIVE_MUTED)))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(px(0.))
                            .gap_1()
                            .pl_3()
                            .when(!forward.content.is_empty(), |body| {
                                body.child(self.rich_embed(
                                    ElementId::Name(format!("{prefix}-text").into()),
                                    &forward.content,
                                    &forward.mentions,
                                    message,
                                    FORWARD_PART,
                                ))
                            })
                            .children(forward.images.iter().enumerate().map(|(n, picture)| {
                                self.render_picture(picture, ElementId::Name(format!("{prefix}-picture-{n}").into()), cx)
                            }))
                            .children(forward.files.iter().enumerate().map(|(n, file)| {
                                self.render_file(file, ElementId::Name(format!("{prefix}-file-{n}").into()), cx)
                            }))
                            .children(
                                forward
                                    .embeds
                                    .iter()
                                    .enumerate()
                                    .filter_map(|(n, embed)| self.render_embed(embed, message, &prefix, n)),
                            )
                            .children(forward.stickers.iter().map(|sticker| self.render_sticker(sticker, 160.)))
                            .when(!footer.is_empty(), |body| {
                                body.child(
                                    div()
                                        .id(ElementId::Name(format!("{prefix}-origin").into()))
                                        .text_xs()
                                        .text_color(theme::muted())
                                        .when_some(target, |line, (guild, channel, original)| {
                                            line.cursor_pointer().hover(|s| s.text_color(theme::text())).on_click(
                                                cx.listener(move |this, _, window, cx| {
                                                    this.open_channel_link(guild, channel, original, window, cx)
                                                }),
                                            )
                                        })
                                        .child(footer),
                                )
                            }),
                    ),
            )
            .into_any_element()
    }
}

/// A round play button centred on media.
fn play_button(size: f32) -> gpui::Div {
    div()
        .flex_none()
        .size(px(size))
        .rounded_full()
        .bg(theme::scrim())
        .flex()
        .items_center()
        .justify_center()
        .child(icons::icon(icons::PLAY, size * 0.5, theme::TEXT_STRONG))
        .map(|button| {
            // Over a picture it sits in the middle; in a card, inline.
            if size > 40. {
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(button)
            } else {
                button
            }
        })
}

/// A small label in the corner of media: "ALT", "GIF".
fn badge(text: &'static str) -> gpui::Div {
    div()
        .absolute()
        .bottom(px(6.))
        .left(px(6.))
        .px_1()
        .rounded_sm()
        .bg(theme::scrim())
        .text_xs()
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(theme::TEXT_STRONG))
        .child(text)
}

/// "0:07" or "12:30", a voice message's length.
fn clock(secs: f64) -> String {
    let secs = secs.max(0.).round() as u64;
    format!("{}:{:02}", secs / 60, secs % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_lengths() {
        assert_eq!(clock(7.4), "0:07");
        assert_eq!(clock(750.), "12:30");
    }
}
