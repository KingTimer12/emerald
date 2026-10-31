//! Pieces of a message row: the reply line, the body, thread chips, stickers, system lines and day separators.

use super::*;
use crate::zoom::zoomed;

impl Emerald {
    /// Discord's reply line: a curved connector into the avatar column, then
    /// the original author and the first line of what they wrote. Clicking
    /// it jumps to the original, like an anchor.
    pub(super) fn render_reply(&self, reply: &Reply, ix: usize, cx: &mut Context<Self>) -> Stateful<Div> {
        let group: SharedString = format!("reply-{ix}").into();
        let line = div()
            .id(("reply", ix))
            .group(group.clone())
            .flex()
            .items_center()
            .gap_1()
            .pl(zoomed(20.))
            .h(zoomed(22.))
            .text_size(zoomed(14.));
        let connector = div()
            .flex_none()
            .w(zoomed(32.))
            .h(zoomed(10.))
            .mt(zoomed(8.))
            .mr_1()
            .border_l_2()
            .border_t_2()
            .rounded_tl(theme::radius::control())
            .border_color(rgb(theme::interactive_muted()));
        let Some(content) = &reply.content else {
            return line.child(connector).child(
                div()
                    .italic()
                    .text_color(theme::muted())
                    .child(t!("chat.reply_deleted")),
            );
        };
        let (name, color) = self.member_style(reply.author_id, &reply.author);
        let line = match reply.id {
            Some(original) => line
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| this.jump_to(original, cx))),
            None => line,
        };
        line.child(connector)
            .child(widgets::avatar(
                &name,
                reply.author_id,
                self.avatar_image(&reply.avatar_url),
                16.,
                None,
            ))
            .child(
                div()
                    .flex_none()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(color.unwrap_or(theme::text_strong())))
                    .child(format!("@{name}")),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(zoomed(0.))
                    .truncate()
                    .text_color(theme::muted())
                    // Brighter on hover, as the web client hints the link.
                    .group_hover(group, |s| s.text_color(theme::text()))
                    .child(self.rich_line(("reply", ix), content, &[])),
            )
    }

    pub(super) fn message_body(&self, msg: &Msg, ix: usize, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .min_w(zoomed(0.))
            .text_color(if msg.pending { theme::muted() } else { theme::text() })
            .map(|body| {
                if self.is_editing(msg) {
                    body.child(self.render_edit_box())
                } else if msg.content.is_empty() {
                    body
                } else {
                    let edited = msg.edited.then(|| t!("chat.edited"));
                    body.child(self.rich_text_noted(("content", ix), &msg.content, &msg.mentions, edited, msg.id))
                }
            })
            .children(msg.forward.as_ref().map(|forward| self.render_forward(forward, msg.id, ix, cx)))
            .children(msg.images.iter().enumerate().map(|(n, picture)| {
                self.render_picture(picture, ElementId::Name(format!("picture-{ix}-{n}").into()), cx)
            }))
            .children(msg.files.iter().enumerate().map(|(n, file)| {
                self.render_file(file, ElementId::Name(format!("file-{ix}-{n}").into()), cx)
            }))
            .children(msg.poll.as_ref().map(|poll| self.render_poll(poll, msg, ix, cx)))
            .children(
                msg.embeds
                    .iter()
                    .enumerate()
                    .filter_map(|(n, embed)| self.render_embed(embed, msg.id, &format!("message-{ix}"), n)),
            )
            .children(self.render_invite_cards(&msg.content, msg.id, ix, cx))
            .children(msg.stickers.iter().map(|sticker| self.render_sticker(sticker, STICKER_SIZE)))
            .children(msg.thread.as_ref().map(|thread| self.render_thread_chip(thread, ix, cx)))
            .children(self.render_reactions(msg, ix, cx))
    }

    /// Under a message that started a thread: its name and how many
    /// messages it has, opening it on click.
    pub(super) fn render_thread_chip(&self, thread: &Channel, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        // The live copy, kept current by THREAD_UPDATE, over the one the
        // message carried when it loaded.
        let live = self.spaces.current().channels().iter().find(|c| c.id == thread.id);
        let count = live.and_then(|c| c.message_count).or(thread.message_count).unwrap_or(0);
        let name: SharedString = live.unwrap_or(thread).name.clone().unwrap_or_default().into();
        let opened = thread.clone();
        div()
            .id(("thread-chip", ix))
            .max_w(zoomed(432.))
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .py_2()
            .rounded(theme::radius::control())
            .cursor_pointer()
            .bg(rgb(theme::base_low()))
            .hover(|s| s.bg(rgb(theme::surface_high())))
            .child(icons::icon(icons::THREAD, 18., theme::interactive()))
            .child(div().min_w(zoomed(0.)).truncate().font_weight(FontWeight::SEMIBOLD).child(name))
            .child(
                div()
                    .flex_none()
                    .text_size(zoomed(14.))
                    .text_color(rgb(theme::link()))
                    .child(t!("thread.messages", count = count)),
            )
            .on_click(cx.listener(move |this, _, window, cx| this.open_thread(opened.clone(), window, cx)))
            .into_any_element()
    }

    /// A sticker drawn `size` pixels square.
    pub(in super::super) fn render_sticker(&self, sticker: &Sticker, size: f32) -> AnyElement {
        let url = sticker.url.as_deref();
        let shown = match url {
            Some(url) if sticker.lottie => self.images.animation(url, &*self.net).and_then(|player| {
                self.animating.set(true);
                let frame = player.frame(self.animation_clock.elapsed())?;
                Some(img(frame).size(zoomed(size)).into_any_element())
            }),
            Some(url) => self.avatar_image(url).map(|image| img(image).size(zoomed(size)).into_any_element()),
            None => None,
        };
        // Unknown formats, and stickers still downloading: a card with the
        // sticker's name in its place.
        shown.unwrap_or_else(|| {
            div()
                        .size(zoomed(size))
                        .rounded(theme::radius::card())
                        .bg(theme::hover())
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap_1()
                        .text_size(zoomed(14.))
                        .text_color(theme::muted())
                        .child(div().text_xs().child(t!("chat.sticker")))
                        .child(div().font_weight(FontWeight::SEMIBOLD).child(sticker.name.clone()))
                        .into_any_element()
        })
    }
}

/// One-line text of a system message, or `None` for a regular one.
pub(super) fn system_text(kind: u8, author: &str) -> Option<String> {
    Some(match kind {
        message_kind::MEMBER_JOIN => t!("chat.member_join", author = author),
        message_kind::PIN => t!("chat.pin", author = author),
        message_kind::BOOST => t!("chat.boost", author = author),
        _ => return None,
    })
}

/// "19 de outubro de 2026" between two hairlines.
pub(in crate::app) fn day_separator(label: String) -> Div {
    let line = || div().flex_1().h(zoomed(1.)).bg(theme::border());
    div()
        .w_full()
        .flex()
        .items_center()
        .gap_1()
        .px_4()
        .mt(zoomed(24.))
        .mb(zoomed(8.))
        .child(line())
        .child(
            div()
                .flex_none()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::muted())
                .child(label),
        )
        .child(line())
}

/// Discord's "SPOILER" label over hidden media.
pub(in super::super::super) fn spoiler_pill() -> gpui::Div {
    div()
        .px_3()
        .py_1()
        .rounded_full()
        .bg(theme::scrim())
        .text_size(zoomed(14.))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(theme::text_strong()))
        .child(t!("chat.spoiler"))
}
