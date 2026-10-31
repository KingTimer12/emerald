//! The Inbox, a popover under its title bar button as in Discord 2026:
//! the channels with unread messages, each a card with those messages, and
//! recent mentions.

mod unreads;

use std::time::Instant;

use rustc_hash::{FxHashMap, FxHashSet};

use super::*;
use super::super::widgets::popover::{self, Anchor};

use emerald_discord::Command;
use emerald_discord::model::Message;
use crate::zoom::zoomed;

/// Mentions come 25 at a time; a full page means there may be more.
const MENTIONS_PAGE: usize = 25;
const WIDTH: f32 = 600.;
const HEIGHT: f32 = 690.;
/// Unread channels whose messages load at a time; "Load more" adds as many.
const UNREADS_PAGE: usize = 5;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in super::super) enum InboxTab {
    Unreads,
    Mentions,
}

pub(in super::super) struct Inbox {
    anchor: Anchor,
    tab: InboxTab,
    mentions: Results,
    /// Each unread channel's messages since the last one read, fetched as
    /// its card is shown.
    unreads: FxHashMap<Snowflake, Results>,
    /// How many unread channels have a card.
    shown: usize,
    /// Cards folded to their header.
    collapsed: FxHashSet<Snowflake>,
    /// Mentions were asked for, which happens on the first visit to the tab.
    mentions_asked: bool,
}

impl Emerald {
    /// Opens the Inbox at `anchor`, or closes it when it is open. A click on
    /// the button just after a press outside closed it does not reopen it.
    pub(in super::super) fn toggle_inbox(&mut self, anchor: Anchor, cx: &mut Context<Self>) {
        let closed = self.inbox_dismissed.take();
        if self.inbox.take().is_none() && !popover::just_closed(closed, Instant::now()) {
            self.inbox = Some(Inbox {
                anchor,
                tab: InboxTab::Unreads,
                mentions: Results::loading(),
                unreads: FxHashMap::default(),
                shown: UNREADS_PAGE,
                collapsed: FxHashSet::default(),
                mentions_asked: false,
            });
            self.load_unreads();
        }
        cx.notify();
    }

    pub(in super::super) fn close_inbox(&mut self, cx: &mut Context<Self>) {
        if self.inbox.take().is_some() {
            cx.notify();
        }
    }

    fn inbox_tab(&mut self, tab: InboxTab, cx: &mut Context<Self>) {
        let Some(inbox) = &mut self.inbox else { return };
        inbox.tab = tab;
        if tab == InboxTab::Mentions && !std::mem::replace(&mut inbox.mentions_asked, true) {
            self.net.post(Command::Mentions { before: None });
        }
        cx.notify();
    }

    fn more_mentions(&mut self, cx: &mut Context<Self>) {
        let Some(inbox) = &mut self.inbox else { return };
        let mentions = &mut inbox.mentions;
        if mentions.loading {
            return;
        }
        mentions.loading = true;
        let before = mentions.items.last().map(|found| found.msg.id);
        self.net.post(Command::Mentions { before });
        cx.notify();
    }

    pub(in super::super) fn on_mentions(&mut self, before: Option<Snowflake>, messages: Result<Vec<Message>, String>) {
        let Some(inbox) = &mut self.inbox else { return };
        let mentions = &mut inbox.mentions;
        if mentions.items.last().map(|found| found.msg.id) != before {
            return;
        }
        mentions.loading = false;
        match messages {
            Ok(messages) => {
                mentions.more = messages.len() >= MENTIONS_PAGE;
                let spaces = &self.spaces;
                mentions.items.extend(messages.into_iter().map(|message| Found::new(message, spaces)));
            }
            Err(error) => mentions.error = Some(error.into()),
        }
    }

    /// The Inbox over everything, `window_height` tall at most.
    pub(in super::super) fn render_inbox(&self, window_height: gpui::Pixels, cx: &mut Context<Self>) -> Option<AnyElement> {
        let inbox = self.inbox.as_ref()?;
        let tab = inbox.tab;
        let tab_button = |id: &'static str, label: &'static str, this_tab: InboxTab, cx: &mut Context<Self>| {
            let on = tab == this_tab;
            div()
                .id(id)
                .flex_1()
                .h(zoomed(40.))
                .flex()
                .items_center()
                .justify_center()
                .border_b_2()
                .border_color(if on { rgb(theme::brand()).into() } else { gpui::transparent_black() })
                .cursor_pointer()
                .text_size(zoomed(14.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(if on { theme::link() } else { theme::interactive() }))
                .hover(|s| s.text_color(rgb(theme::text_strong())))
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| this.inbox_tab(this_tab, cx)))
        };
        let body = match tab {
            InboxTab::Unreads => self.render_unreads(inbox, cx),
            InboxTab::Mentions => {
                self.render_results("mentions", &inbox.mentions, t!("inbox.no_mentions"), true, Self::more_mentions, cx)
            }
        };
        let header = div()
            .flex_none()
            .px_4()
            .pt_4()
            .pb_2()
            .flex()
            .items_center()
            .gap_2()
            .child(icons::icon(icons::INBOX, 24., theme::text_strong()))
            .child(
                div()
                    .flex_1()
                    .text_size(zoomed(20.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::text_strong()))
                    .child(t!("inbox.title")),
            )
            .when(tab == InboxTab::Unreads, |header| {
                header.child(
                    square_button("inbox-read-all", icons::CHECK_DOUBLE)
                        .tooltip(widgets::tooltip(t!("inbox.mark_all_read")))
                        .on_click(cx.listener(|this, _, _, cx| this.mark_inbox_read(cx))),
                )
            });
        let panel = div()
            .id("inbox")
            .w(zoomed(WIDTH))
            .h(zoomed(HEIGHT))
            .max_h(window_height - zoomed(48.))
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(theme::radius::modal())
            .border_1()
            .border_color(theme::border_normal())
            .bg(rgb(theme::surface_high()))
            .shadow_lg()
            .occlude()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                // A menu the Inbox opened lies outside it; it closes itself.
                if this.place_menu.is_some() {
                    return;
                }
                this.inbox_dismissed = this.inbox.take().map(|_| Instant::now());
                cx.notify();
            }))
            .child(header)
            .child(
                div()
                    .flex_none()
                    .flex()
                    .border_b_1()
                    .border_color(theme::border_normal())
                    .child(tab_button("inbox-unreads", t!("inbox.unreads"), InboxTab::Unreads, cx))
                    .child(tab_button("inbox-mentions", t!("inbox.mentions"), InboxTab::Mentions, cx)),
            )
            .child(body);
        Some(popover::at(inbox.anchor, panel))
    }
}

/// A 32px bordered square with an icon, as at the right of the Inbox's
/// title and of its cards.
fn square_button(id: impl Into<gpui::ElementId>, glyph: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .size(zoomed(32.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme::radius::control())
        .border_1()
        .border_color(theme::border_normal())
        .bg(rgb(theme::base_low()))
        .cursor_pointer()
        .hover(|s| s.bg(theme::hover()))
        .child(icons::icon(glyph, 18., theme::interactive()))
}
