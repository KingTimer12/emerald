//! The chat list's rows: the channel intro, messages with their replies,
//! attachments, embeds and stickers, and day separators.

use gpui::{AnyElement, Div, FontWeight, HighlightStyle, ListOffset, ListState, SharedString, StyledText, div, img, prelude::*, px, rgb};

use super::super::super::{icons, markdown, theme, time};
use super::super::state::{Log, MessageView, Msg, Reply};
use super::super::{Emerald, widgets};
use crate::discord::model::{Embed, message_kind};

/// Rows above the messages: the channel intro once the whole history is
/// loaded, an empty spacer before that.
const TOP: usize = 1;
/// Messages from one author closer than this share a group, like Discord.
const GROUP_WINDOW_SECS: i64 = 7 * 60;
const STICKER_SIZE: f32 = 160.;

/// The chat list mirrors the open log one row per message, below `TOP`.
impl MessageView for ListState {
    fn reset(&self, messages: usize) {
        ListState::reset(self, TOP + messages);
    }

    fn insert(&self, at: usize, count: usize) {
        self.splice(TOP + at..TOP + at, count);
    }

    fn remove(&self, at: usize) {
        self.splice(TOP + at..TOP + at + 1, 0);
    }

    fn refresh(&self, at: usize) {
        self.splice(TOP + at..TOP + at + 1, 1);
    }

    fn prepend(&self, count: usize) {
        let top = self.logical_scroll_top();
        self.splice(TOP..TOP, count);
        self.scroll_to(ListOffset {
            item_ix: top.item_ix + count,
            offset_in_item: top.offset_in_item,
        });
    }

    fn refresh_intro(&self) {
        self.splice(0..TOP, TOP);
    }
}

impl Emerald {
    /// Row `ix` of the chat list: the intro on top, then the messages.
    pub(super) fn render_list_row(&self, ix: usize) -> AnyElement {
        let log = self.selected_channel.and_then(|c| self.history.get(c));
        if ix < TOP {
            return self.render_intro(log);
        }
        match log {
            Some(log) if ix - TOP < log.messages.len() => self.render_message(log, ix - TOP),
            _ => div().into_any_element(),
        }
    }

    /// Discord's "beginning of the conversation" block, shown once the
    /// history is loaded to its start.
    fn render_intro(&self, log: Option<&Log>) -> AnyElement {
        if !log.is_some_and(|log| log.complete) {
            return div().h(px(1.)).into_any_element();
        }
        let space = self.spaces.current();
        let Some(row) = self.selected_channel.and_then(|id| space.row(id)) else {
            return div().h(px(1.)).into_any_element();
        };
        let (icon, title, user) = (row.icon, &row.name, row.user);
        let intro = div().w_full().flex().flex_col().px_4().pt_4();
        let heading = |text: SharedString, size: f32| {
            div()
                .mt_2()
                .text_size(px(size))
                .line_height(px((size * 1.25).round()))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(theme::TEXT_STRONG))
                .child(text)
        };

        if space.is_dms() {
            let username = user
                .and_then(|user| self.people.user(user))
                .map(|u| SharedString::from(u.username.clone()));
            let prefix = "Este é o começo do seu histórico de mensagens diretas com ";
            let text = format!("{prefix}{title}.");
            let name = prefix.len()..prefix.len() + title.len();
            intro
                .children(user.map(|user| widgets::avatar(title, user, self.user_avatar(user), 80., None)))
                .child(heading(title.clone(), 30.))
                .children(username.map(|username| {
                    div()
                        .text_size(px(24.))
                        .line_height(px(28.))
                        .text_color(rgb(theme::TEXT_STRONG))
                        .child(username)
                }))
                .child(
                    div().mt_2().text_color(theme::muted()).child(StyledText::new(text).with_highlights([(
                        name,
                        HighlightStyle {
                            font_weight: Some(FontWeight::BOLD),
                            color: Some(rgb(theme::TEXT_STRONG).into()),
                            ..Default::default()
                        },
                    )])),
                )
                .into_any_element()
        } else {
            intro
                .child(
                    div()
                        .size(px(68.))
                        .rounded_full()
                        .bg(theme::hover())
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icons::icon(icon, 44., theme::TEXT_STRONG)),
                )
                .child(heading(format!("Boas-vindas a #{title}!").into(), 32.))
                .child(
                    div()
                        .mt_1()
                        .text_color(theme::muted())
                        .child(format!("Este é o começo do canal #{title}.")),
                )
                .into_any_element()
        }
    }

    fn render_message(&self, log: &Log, ix: usize) -> AnyElement {
        let msg = &log.messages[ix];
        let previous = ix.checked_sub(1).map(|ix| &log.messages[ix]);
        let new_day = previous.is_none_or(|p| p.day != msg.day);
        let (author, author_color) = self.author_style(msg);
        let row = div()
            .id(("message", ix))
            .w_full()
            .hover(|s| s.bg(theme::hover()));

        let message = if let Some(text) = system_text(msg.kind, &author) {
            // System line: arrow in the avatar column, then the event.
            row.flex()
                .items_center()
                .gap_4()
                .px_4()
                .py(px(2.))
                .when(!new_day, |r| r.mt(px(15.)))
                .child(
                    div()
                        .w(px(40.))
                        .flex_none()
                        .flex()
                        .justify_center()
                        .text_color(rgb(theme::ONLINE))
                        .child("→"),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .text_color(theme::muted())
                        .child(text),
                )
                .child(
                    div()
                        .flex_none()
                        .text_xs()
                        .text_color(theme::muted())
                        .child(msg.time.clone()),
                )
                .into_any_element()
        } else if self.settings.compact {
            // Compact mode: one line per message, no avatars or group gaps.
            row.flex()
                .gap_2()
                .px_4()
                .py(px(1.))
                .child(
                    div()
                        .flex_none()
                        .text_xs()
                        .text_color(theme::muted())
                        .child(msg.short_time.clone()),
                )
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .min_w(px(0.))
                        .gap_1()
                        .child(
                            div()
                                .flex_none()
                                .text_color(rgb(author_color))
                                .font_weight(FontWeight::MEDIUM)
                                .child(author),
                        )
                        .child(self.message_body(msg, ix)),
                )
                .into_any_element()
        } else if !new_day
            && msg.reply.is_none()
            && previous.is_some_and(|p| {
                p.author_id == msg.author_id
                    && msg.secs - p.secs < GROUP_WINDOW_SECS
                    && system_text(p.kind, "").is_none()
            })
        {
            row.pl(px(72.))
                .pr_4()
                .py(px(2.))
                .child(self.message_body(msg, ix))
                .into_any_element()
        } else {
            // Group start: 17px gap above, outside the hover highlight. A day
            // separator already provides the gap.
            div()
                .w_full()
                .when(!new_day, |d| d.pt(px(15.)))
                .child(
                    row.flex()
                        .flex_col()
                        .px_4()
                        .py(px(2.))
                        .children(msg.reply.as_ref().map(|reply| self.render_reply(reply)))
                        .child(
                            div()
                                .flex()
                                .gap_4()
                                .child(div().mt(px(2.)).child(widgets::avatar(
                                    &author,
                                    msg.author_id,
                                    self.avatar_image(&msg.avatar_url),
                                    40.,
                                    None,
                                )))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .flex_1()
                                        .min_w(px(0.))
                                        .child(
                                            div()
                                                .flex()
                                                .gap(px(6.))
                                                .items_center()
                                                .child(
                                                    div()
                                                        .text_color(rgb(author_color))
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .child(author),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(theme::muted())
                                                        .child(msg.time.clone()),
                                                ),
                                        )
                                        .child(self.message_body(msg, ix)),
                                ),
                        ),
                )
                .into_any_element()
        };

        if !new_day {
            return message;
        }
        div()
            .w_full()
            .flex()
            .flex_col()
            .child(day_separator(time::day_label(msg.secs)))
            .child(message)
            .into_any_element()
    }

    /// Discord's reply line: a curved connector into the avatar column, then
    /// the original author and the first line of what they wrote.
    fn render_reply(&self, reply: &Reply) -> Div {
        let line = div().flex().items_center().gap_1().pl(px(20.)).h(px(22.)).text_size(px(14.));
        let connector = div()
            .flex_none()
            .w(px(32.))
            .h(px(10.))
            .mt(px(8.))
            .mr_1()
            .border_l_2()
            .border_t_2()
            .rounded_tl(px(6.))
            .border_color(rgb(theme::INTERACTIVE_MUTED));
        let Some(content) = &reply.content else {
            return line.child(connector).child(
                div()
                    .italic()
                    .text_color(theme::muted())
                    .child("A mensagem original foi apagada"),
            );
        };
        let (name, color) = self.member_style(reply.author_id, &reply.author);
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
                    .text_color(rgb(color.unwrap_or(theme::TEXT_STRONG)))
                    .child(format!("@{name}")),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .truncate()
                    .text_color(theme::muted())
                    .child(self.rich_text(("reply", 0usize), content, &[], false)),
            )
    }

    fn message_body(&self, msg: &Msg, ix: usize) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .min_w(px(0.))
            .text_color(if msg.pending { theme::muted() } else { theme::text() })
            .when(!msg.content.is_empty(), |body| {
                body.child(self.rich_text(("content", ix), &msg.content, &msg.mentions, true))
            })
            .children(msg.images.iter().map(|(url, w, h)| {
                let frame = div().size_full().rounded_lg().overflow_hidden();
                div()
                    .w(px(*w))
                    .h(px(*h))
                    .child(match self.avatar_image(url) {
                        Some(image) => frame.child(img(image).size_full()),
                        None => frame.bg(theme::hover()),
                    })
            }))
            .children(msg.files.iter().map(|name| {
                div()
                    .max_w(px(432.))
                    .flex()
                    .items_center()
                    .gap_2()
                    .p_3()
                    .rounded_lg()
                    .border_1()
                    .border_color(theme::border())
                    .bg(rgb(theme::BASE_LOW))
                    .child(div().text_size(px(24.)).child("📄"))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .text_color(rgb(theme::LINK))
                            .child(name.clone()),
                    )
            }))
            .children(msg.embeds.iter().enumerate().filter_map(|(n, embed)| self.render_embed(embed, ix, n)))
            .children(msg.stickers.iter().map(|(name, url)| {
                match url.as_ref().and_then(|url| self.avatar_image(url)) {
                    Some(image) => img(image).size(px(STICKER_SIZE)).into_any_element(),
                    // Lottie stickers, and images still downloading: a card
                    // with the sticker's name in its place.
                    None => div()
                        .size(px(STICKER_SIZE))
                        .rounded_lg()
                        .bg(theme::hover())
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap_1()
                        .text_size(px(14.))
                        .text_color(theme::muted())
                        .child(div().text_xs().child("Figurinha"))
                        .child(div().font_weight(FontWeight::SEMIBOLD).child(name.clone()))
                        .into_any_element(),
                }
            }))
    }

    /// Text parts of a rich embed in a card with its color bar; embeds with
    /// nothing textual (bare image previews) are skipped.
    fn render_embed(&self, embed: &Embed, message: usize, n: usize) -> Option<Div> {
        let provider = embed.provider.as_ref().and_then(|p| p.name.clone());
        let author = embed.author.as_ref().and_then(|a| a.name.clone());
        let footer = embed.footer.as_ref().and_then(|f| f.text.clone());
        if embed.title.is_none() && embed.description.is_none() && author.is_none() {
            return None;
        }
        let small = |text: String, size: f32, color: u32| {
            div().text_size(px(size)).text_color(rgb(color)).child(text)
        };
        let title = embed.title.clone().map(|title| {
            // Embed URLs come from the sender; open only http(s).
            let url = embed.url.as_deref().and_then(markdown::web_url).map(str::to_owned);
            let link = url.is_some();
            let element = div()
                .text_size(px(16.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(if link { theme::LINK } else { theme::TEXT_STRONG }))
                .child(title);
            match url {
                Some(url) => element
                    .id(("embed-title", message * 16 + n))
                    .cursor_pointer()
                    .on_click(move |_, _, cx| cx.open_url(&url))
                    .into_any_element(),
                None => element.into_any_element(),
            }
        });
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
                        .gap_1()
                        .px_3()
                        .py_2()
                        .children(provider.map(|p| small(p, 12., theme::TEXT_MUTED)))
                        .children(author.map(|a| {
                            small(a, 14., theme::TEXT_STRONG).font_weight(FontWeight::SEMIBOLD)
                        }))
                        .children(title)
                        .children(embed.description.as_ref().map(|description| {
                            div()
                                .text_size(px(14.))
                                .line_height(px(18.))
                                .child(self.rich_text(("embed", message * 16 + n), description, &[], true))
                        }))
                        .children(footer.map(|f| small(f, 12., theme::TEXT_MUTED))),
                ),
        )
    }
}

/// One-line text of a system message, or `None` for a regular one.
fn system_text(kind: u8, author: &str) -> Option<String> {
    Some(match kind {
        message_kind::MEMBER_JOIN => format!("{author} entrou no servidor."),
        message_kind::PIN => format!("{author} fixou uma mensagem neste canal."),
        message_kind::BOOST => format!("{author} impulsionou o servidor!"),
        _ => return None,
    })
}

/// "19 de outubro de 2026" between two hairlines.
fn day_separator(label: String) -> Div {
    let line = || div().flex_1().h(px(1.)).bg(theme::border());
    div()
        .w_full()
        .flex()
        .items_center()
        .gap_1()
        .px_4()
        .mt(px(24.))
        .mb(px(8.))
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
