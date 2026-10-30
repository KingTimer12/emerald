//! Moving through a channel's history: jumping to a message (a reply's
//! original, a link) with the page around it, to the first unread one, and
//! back to the present; the "new messages" bar and divider, and the bar
//! that offers the way back when reading older messages.

use std::time::Duration;

use gpui::{AnyElement, Context, FontWeight, ListOffset, div, prelude::*, rgb};

use super::super::super::{theme, time};
use super::super::Emerald;
use super::list::TOP;
use emerald_discord::model::Snowflake;
use emerald_discord::{Command, Page};
use crate::zoom::zoomed;

/// How long a message jumped to stays highlighted.
const FLASH: Duration = Duration::from_secs(2);
/// Scrolled up past this many rows from the bottom, the way back to the
/// present is offered.
const FAR_FROM_PRESENT: usize = 40;

/// Where a jump in a channel is headed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JumpTarget {
    Message(Snowflake),
    /// The first message after this one, the last read.
    FirstUnread(Snowflake),
}

impl JumpTarget {
    /// The message whose page holds the target.
    fn anchor(self) -> Snowflake {
        match self {
            Self::Message(id) | Self::FirstUnread(id) => id,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Jump {
    pub channel: Snowflake,
    pub target: JumpTarget,
    /// The page around the target was asked for; if it is still not there
    /// when the page comes, it is gone.
    pub asked: bool,
}

impl Emerald {
    /// Scrolls to `message` in the open channel and flashes it, loading the
    /// page around it first when it is not in, as clicking a reply does.
    pub(in super::super) fn jump_to(&mut self, message: Snowflake, cx: &mut Context<Self>) {
        let Some(channel) = self.selected_channel else { return };
        self.start_jump(channel, JumpTarget::Message(message));
        self.continue_jump(channel, cx);
    }

    /// Sets a jump in `channel` going, before or after opening it.
    pub(in super::super) fn start_jump(&mut self, channel: Snowflake, target: JumpTarget) {
        self.jump = Some(Jump {
            channel,
            target,
            asked: false,
        });
    }

    /// The page to open `channel` with: around a jump's target when one is
    /// waiting for it, the newest messages otherwise.
    pub(in super::super) fn first_page(&mut self, channel: Snowflake) -> Page {
        match &mut self.jump {
            Some(jump) if jump.channel == channel => {
                jump.asked = true;
                Page::Around(jump.target.anchor())
            }
            _ => Page::Latest,
        }
    }

    /// Advances a pending jump in `channel`: reveals the target once its
    /// page is in, asks for the page around it otherwise, and gives up when
    /// that page comes without it.
    pub(in super::super) fn continue_jump(&mut self, channel: Snowflake, cx: &mut Context<Self>) {
        let Some(jump) = self.jump.filter(|jump| jump.channel == channel) else { return };
        if self.selected_channel != Some(channel) {
            self.jump = None;
            return;
        }
        if let Some((ix, flash)) = self.find_target(channel, jump) {
            self.jump = None;
            // A couple of rows above it stay in view, for context.
            self.messages.scroll_to(ListOffset {
                item_ix: (TOP + ix).saturating_sub(2),
                offset_in_item: zoomed(0.),
            });
            if let Some(message) = flash {
                self.flash_message(message, cx);
            }
            cx.notify();
            return;
        }
        // A page on its way first; this runs again when it lands.
        if self.history.get(channel).is_some_and(|log| log.loading) {
            return;
        }
        if jump.asked || !self.history.start_loading(channel) {
            // The page around it came and it was not there.
            self.jump = None;
            if matches!(jump.target, JumpTarget::Message(_)) {
                self.connection.set_message(t!("chat.jump_failed"));
            }
            cx.notify();
            return;
        }
        self.jump = Some(Jump { asked: true, ..jump });
        self.net.post(Command::History {
            channel,
            page: Page::Around(jump.target.anchor()),
        });
        cx.notify();
    }

    /// Where the target of `jump` is in the log, and the message to flash.
    /// The first unread is only certain once the log holds the last read
    /// message or something older, or reaches the start.
    fn find_target(&self, channel: Snowflake, jump: Jump) -> Option<(usize, Option<Snowflake>)> {
        match jump.target {
            JumpTarget::Message(id) => self.history.find(channel, id).map(|(ix, _)| (ix, Some(id))),
            JumpTarget::FirstUnread(read) => {
                let log = self.history.get(channel)?;
                let covered = log.complete || jump.asked || log.messages.front().is_some_and(|m| m.id <= read);
                let ix = log.messages.iter().position(|m| m.id > read).filter(|_| covered)?;
                Some((ix, None))
            }
        }
    }

    fn flash_message(&mut self, message: Snowflake, cx: &mut Context<Self>) {
        self.flash = Some(message);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(FLASH).await;
            this.update(cx, |this, cx| {
                if this.flash == Some(message) {
                    this.flash = None;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Back to the newest messages: a scroll when the log reaches them, a
    /// fresh page when it was left behind by a jump.
    pub(in super::super) fn jump_to_present(&mut self, cx: &mut Context<Self>) {
        let Some(channel) = self.selected_channel else { return };
        self.jump = None;
        if self.history.at_latest(channel) {
            let count = self.history.get(channel).map_or(0, |log| log.messages.len());
            // Bottom-aligned, a fresh list shows its end.
            self.messages.reset(TOP + count);
        } else if self.history.start_loading(channel) {
            self.net.post(Command::History {
                channel,
                page: Page::Latest,
            });
        }
        cx.notify();
    }

    /// The list scrolled to its last rows: pages in newer messages when
    /// the log was left behind by a jump.
    pub(in super::super) fn on_list_scrolled(&mut self, end: usize, count: usize) {
        let Some(channel) = self.selected_channel else { return };
        if end >= count
            && let Some(newest) = self.history.newer(channel)
        {
            self.net.post(Command::History {
                channel,
                page: Page::After(newest),
            });
        }
    }

    /// Row `ix` of the list was drawn: when it is the divider, the unread
    /// messages are in sight and their bar can go.
    pub(super) fn row_drawn(&mut self, ix: usize, cx: &mut Context<Self>) {
        if !self.divider_seen && self.divider_row() == Some(ix) {
            self.divider_seen = true;
            cx.notify();
        }
    }

    /// The list row of the first unread message, if it is loaded.
    fn divider_row(&self) -> Option<usize> {
        let (channel, read) = self.unread_marker?;
        let log = self.history.get(channel)?;
        let ix = log.messages.iter().position(|m| m.id > read)?;
        // Only when what came before it is loaded too.
        (ix > 0 || log.complete).then_some(TOP + ix)
    }

    /// Whether the divider goes above message `ix` of the open log: it is
    /// the first after the last read one.
    pub(super) fn starts_unread(&self, ix: usize) -> bool {
        self.divider_row() == Some(TOP + ix)
    }

    /// The bar over the top of the list while unread messages sit above
    /// what is on screen: how many, since when, a click to go to them and
    /// a button to drop the bar.
    pub(super) fn render_unread_bar(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (channel, read) = self.unread_marker.filter(|(c, _)| Some(*c) == self.selected_channel)?;
        let log = self.history.get(channel)?;
        let unread: Vec<_> = log.messages.iter().filter(|m| m.id > read).collect();
        let first = unread.first()?;
        if self.divider_seen {
            return None;
        }
        let all_loaded = log.complete || log.messages.front().is_some_and(|m| m.id <= read);
        let since = time::header(first.secs);
        let text = if all_loaded {
            t!("chat.new_messages", count = unread.len(), time = since)
        } else {
            t!("chat.new_messages_many", count = unread.len(), time = since)
        };
        Some(
            div()
                .absolute()
                .top_0()
                .left(zoomed(16.))
                .right(zoomed(16.))
                .h(zoomed(32.))
                .flex()
                .items_center()
                .rounded_b(theme::radius::card())
                .bg(rgb(theme::brand()))
                .text_size(zoomed(14.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(theme::on_color()))
                .child(
                    div()
                        .id("unread-bar")
                        .flex_1()
                        .h_full()
                        .flex()
                        .items_center()
                        .px_3()
                        .cursor_pointer()
                        .child(text)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.start_jump(channel, JumpTarget::FirstUnread(read));
                            this.continue_jump(channel, cx);
                        })),
                )
                .child(
                    div()
                        .id("unread-bar-read")
                        .h_full()
                        .flex()
                        .items_center()
                        .px_3()
                        .cursor_pointer()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(t!("chat.mark_read"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.unread_marker = None;
                            cx.notify();
                        })),
                )
                .into_any_element(),
        )
    }

    /// The bar over the bottom of the list while reading older messages,
    /// with the way back to the present.
    pub(super) fn render_present_bar(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let channel = self.selected_channel?;
        // At the bottom the list reports its end as the top row.
        let top = self.messages.logical_scroll_top().item_ix;
        let scrolled_up = top + FAR_FROM_PRESENT < self.messages.item_count();
        if self.history.at_latest(channel) && !scrolled_up {
            return None;
        }
        Some(
            div()
                .absolute()
                .bottom_0()
                .left(zoomed(16.))
                .right(zoomed(16.))
                .h(zoomed(32.))
                .flex()
                .items_center()
                .justify_between()
                .px_3()
                .rounded_t(theme::radius::card())
                .bg(rgb(theme::base_lowest()))
                .border_1()
                .border_color(theme::border())
                .text_size(zoomed(14.))
                .child(div().text_color(theme::muted()).child(t!("chat.older_messages")))
                .child(
                    div()
                        .id("jump-present")
                        .cursor_pointer()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(theme::link()))
                        .child(t!("chat.jump_present"))
                        .on_click(cx.listener(|this, _, _, cx| this.jump_to_present(cx))),
                )
                .into_any_element(),
        )
    }
}

/// The red line with "NEW" above the first unread message.
pub(super) fn unread_divider() -> AnyElement {
    let red = rgb(theme::danger());
    div()
        .w_full()
        .flex()
        .items_center()
        .pl_4()
        .my_1()
        .child(div().flex_1().h(zoomed(1.)).bg(red))
        .child(
            div()
                .flex_none()
                .px_1()
                .rounded_l(theme::radius::tag())
                .bg(red)
                .text_size(zoomed(10.))
                .line_height(zoomed(13.))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(theme::text_strong()))
                .child(t!("chat.new")),
        )
        .into_any_element()
}
