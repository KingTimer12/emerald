//! The chat list's rows: the channel intro, messages with their replies,
//! attachments, embeds and stickers, and day separators.

mod parts;

use parts::*;
pub(in super::super) use parts::spoiler_pill;

use gpui::{
    AnyElement, Context, Div, ElementId, FontWeight, HighlightStyle, ListOffset, ListState, MouseButton, SharedString,
    Stateful, StyledText, div, img, prelude::*, px, rgb,
};

use super::super::super::{icons, theme, time};
use super::super::state::{Log, MessageView, Msg, Reply, Sticker};
use super::super::{Emerald, widgets};
use super::jump::unread_divider;
use emerald_discord::model::{Channel, message_kind};

/// Rows above the messages: the channel intro once the whole history is
/// loaded, an empty spacer before that.
pub(super) const TOP: usize = 1;
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
    pub(super) fn render_list_row(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let log = self.selected_channel.and_then(|c| self.history.get(c));
        if ix < TOP {
            return self.render_intro(log);
        }
        match log {
            Some(log) if ix - TOP < log.messages.len() => self.render_message(log, ix - TOP, cx),
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
            let (text, names) = t_spans!("chat.dm_start", name = title);
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
                    div().mt_2().text_color(theme::muted()).child(StyledText::new(text).with_highlights(names.into_iter().map(|name| {
                        (
                            name,
                            HighlightStyle {
                                font_weight: Some(FontWeight::BOLD),
                                color: Some(rgb(theme::TEXT_STRONG).into()),
                                ..Default::default()
                            },
                        )
                    }))),
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
                .child(heading(t!("chat.welcome", name = title).into(), 32.))
                .child(
                    div()
                        .mt_1()
                        .text_color(theme::muted())
                        .child(t!("chat.channel_start", name = title)),
                )
                .into_any_element()
        }
    }

    fn render_message(&self, log: &Log, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let msg = &log.messages[ix];
        let previous = ix.checked_sub(1).map(|ix| &log.messages[ix]);
        let new_day = previous.is_none_or(|p| p.day != msg.day);
        let (author, author_color) = self.author_style(msg);
        let my_roles = self.spaces.current().info().map_or(&[][..], |g| g.my_roles());
        let mentioned = msg.mentions_user(self.people.my_id(), my_roles);
        let editing = self.is_editing(msg);
        let flashed = self.flash == Some(msg.id);
        let group: SharedString = format!("message-{ix}").into();
        let toolbar = self.message_toolbar(msg, ix, group.clone(), cx);
        // Pings get Discord's gold tint and a bar on the left edge.
        let row = div()
            .id(("message", ix))
            .group(group)
            .relative()
            .w_full()
            .when(mentioned, |row| {
                row.bg(theme::mentioned()).child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .bottom_0()
                        .w(px(2.))
                        .bg(rgb(theme::MENTIONED_BAR)),
                )
            })
            .when(editing, |row| row.bg(theme::hover()))
            .when(flashed, |row| row.bg(theme::jumped()))
            .hover(move |s| s.bg(if mentioned { theme::mentioned_hover() } else { theme::hover() }))
            .when(!msg.pending, |row| {
                row.on_mouse_down(MouseButton::Right, self.on_message_right_click(ix, cx))
            });

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
                .children(toolbar)
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
                        .child(self.message_body(msg, ix, cx)),
                )
                .children(toolbar)
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
                .child(self.message_body(msg, ix, cx))
                .children(toolbar)
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
                        .children(msg.reply.as_ref().map(|reply| self.render_reply(reply, ix, cx)))
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
                                        .child(self.message_body(msg, ix, cx)),
                                ),
                        )
                        .children(toolbar),
                )
                .into_any_element()
        };

        let divider = self.starts_unread(ix).then(unread_divider);
        if !new_day && divider.is_none() {
            return message;
        }
        div()
            .w_full()
            .flex()
            .flex_col()
            .when(new_day, |column| column.child(day_separator(time::day_label(msg.secs))))
            .children(divider)
            .child(message)
            .into_any_element()
    }
}
