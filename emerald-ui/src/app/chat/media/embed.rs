//! Embeds: link previews and rich cards, their fields and media.

use super::*;

impl Emerald {
    /// An embed: an image or GIF preview on its own, or a card with the
    /// color bar holding provider, author, title, description, fields,
    /// thumbnail and image. Embeds with nothing to show are skipped.
    pub(in super::super) fn render_embed(&self, embed: &Embed, message: Snowflake, prefix: &str, n: usize) -> Option<AnyElement> {
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
    pub(super) fn render_fields(
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
    pub(super) fn render_embed_media(&self, media: &EmbedMedia, gif: bool, opens: Option<String>, id: ElementId) -> AnyElement {
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
    pub(super) fn animation(&self, url: &str, width: f32, height: f32) -> Option<std::sync::Arc<gpui::RenderImage>> {
        if !self.window_active {
            return None;
        }
        self.images.animated(url, (width * 2.) as u32, (height * 2.) as u32, &*self.net)
    }
}
