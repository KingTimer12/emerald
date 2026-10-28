//! The chat list's rows: the channel intro, messages with their replies,
//! attachments, embeds and stickers, and day separators.

use crate::zoom::zoomed;

mod parts;

use parts::*;
pub(in super::super) use parts::spoiler_pill;

use gpui::{
    AnyElement, Context, Div, ElementId, FontWeight, HighlightStyle, ListOffset, ListState, MouseButton, SharedString,
    Stateful, StyledText, div, img, prelude::*, rgb,
};

use super::super::super::{icons, theme, time};
use super::super::state::hidden::{self, Fold};
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
            return div().h(zoomed(1.)).into_any_element();
        }
        let space = self.spaces.current();
        let Some(row) = self.selected_channel.and_then(|id| space.row(id)) else {
            return div().h(zoomed(1.)).into_any_element();
        };
        let (icon, title, user) = (row.icon, &row.name, row.user);
        let intro = div().w_full().flex().flex_col().px_4().pt_4();
        let heading = |text: SharedString, size: f32| {
            div()
                .mt_2()
                .text_size(zoomed(size))
                .line_height(zoomed((size * 1.25).round()))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(theme::text_strong()))
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
                        .text_size(zoomed(24.))
                        .line_height(zoomed(28.))
                        .text_color(rgb(theme::text_strong()))
                        .child(username)
                }))
                .child(
                    div().mt_2().text_color(theme::muted()).child(StyledText::new(text).with_highlights(names.into_iter().map(|name| {
                        (
                            name,
                            HighlightStyle {
                                font_weight: Some(FontWeight::BOLD),
                                color: Some(rgb(theme::text_strong()).into()),
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
                        .size(zoomed(68.))
                        .rounded_full()
                        .bg(theme::hover())
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icons::icon(icon, 44., theme::text_strong())),
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
        let fold = hidden::fold(
            ix,
            log.messages.len(),
            |ix| (log.messages[ix].id, log.messages[ix].author_id),
            |author| self.people.is_hidden(author),
            |head| self.revealed_hidden.contains(&head),
        );
        match fold {
            Fold::Folded => return div().into_any_element(),
            Fold::Head { count } => return self.render_hidden_bar(msg, count, ix, cx),
            Fold::Shown => {}
        }
        let previous = ix.checked_sub(1).map(|ix| &log.messages[ix]);
        let new_day = previous.is_none_or(|p| p.day != msg.day);
        let (author, author_color) = self.author_style(msg);
        let my_roles = self.spaces.current().info().map_or(&[][..], |g| g.my_roles());
        let mentioned = msg.mentions_user(self.people.my_id(), my_roles);
        let editing = self.is_editing(msg);
        let flashed = self.flash == Some(msg.id);
        let group: SharedString = format!("message-{ix}").into();
        let toolbar = self.message_toolbar(msg, ix, group.clone(), cx);
        let author_id = msg.author_id;
        let space = self.spaces.current();
        let guild = (!space.is_dms()).then_some(space.id);
        let dm_group = self
            .selected_channel
            .filter(|c| self.spaces.channel(*c).is_some_and(|c| c.kind == emerald_discord::model::ChannelKind::GROUP_DM));
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
                        .w(zoomed(2.))
                        .bg(rgb(theme::mentioned_bar())),
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
                .py(zoomed(2.))
                .when(!new_day, |r| r.mt(zoomed(15.)))
                .child(
                    div()
                        .w(zoomed(40.))
                        .flex_none()
                        .flex()
                        .justify_center()
                        .text_color(rgb(theme::online()))
                        .child("→"),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(zoomed(0.))
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
                .py(zoomed(1.))
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
                        .min_w(zoomed(0.))
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
            row.pl(zoomed(72.))
                .pr_4()
                .py(zoomed(2.))
                .child(self.message_body(msg, ix, cx))
                .children(toolbar)
                .into_any_element()
        } else {
            // Group start: 17px gap above, outside the hover highlight. A day
            // separator already provides the gap.
            div()
                .w_full()
                .when(!new_day, |d| d.pt(zoomed(15.)))
                .child(
                    row.flex()
                        .flex_col()
                        .px_4()
                        .py(zoomed(2.))
                        .children(msg.reply.as_ref().map(|reply| self.render_reply(reply, ix, cx)))
                        .child(
                            div()
                                .flex()
                                .gap_4()
                                .child(
                                    div()
                                        .id(("author-avatar", ix))
                                        .mt(zoomed(2.))
                                        .cursor_pointer()
                                        .child(widgets::avatar(
                                            &author,
                                            msg.author_id,
                                            self.avatar_image(&msg.avatar_url),
                                            40.,
                                            None,
                                        ))
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.open_profile(author_id, guild, window, cx)
                                        }))
                                        .on_mouse_down(MouseButton::Right, self.on_user_right_click(author_id, guild, dm_group, cx)),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .flex_1()
                                        .min_w(zoomed(0.))
                                        .child(
                                            div()
                                                .flex()
                                                .gap(zoomed(6.))
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

    /// "2 blocked messages — Show messages": a run of messages from people
    /// the user blocked or ignored, folded until asked, as on Discord. An
    /// opened run shows each message with the bar's "Collapse" on top.
    fn render_hidden_bar(&self, msg: &Msg, count: usize, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let head = msg.id;
        let label = match (self.people.is_blocked(msg.author_id), count) {
            (true, 1) => t!("block.one_message").to_owned(),
            (true, _) => t!("block.messages", count = count),
            (false, 1) => t!("block.one_ignored").to_owned(),
            (false, _) => t!("block.ignored", count = count),
        };
        div()
            .w_full()
            .px_4()
            .py_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(zoomed(14.))
                    .text_color(theme::muted())
                    .child(label)
                    .child("—")
                    .child(
                        div()
                            .id(("show-hidden", ix))
                            .cursor_pointer()
                            .text_color(rgb(theme::link()))
                            .hover(|s| s.underline())
                            .child(if count == 1 { t!("block.show_one") } else { t!("block.show") })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.revealed_hidden.insert(head);
                                if let Some(channel) = this.selected_channel {
                                    let view = super::super::shown(&this.messages, this.selected_channel, channel);
                                    this.history.redraw_all(channel, view);
                                }
                                cx.notify();
                            })),
                    ),
            )
            .into_any_element()
    }
}
