//! The inbox: recent mentions from Discord, and the channels with unread
//! messages, each to open or mark read.

use super::*;

use emerald_discord::Command;
use emerald_discord::model::Message;
use crate::zoom::zoomed;

/// Mentions come 25 at a time; a full page means there may be more.
const MENTIONS_PAGE: usize = 25;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in super::super) enum InboxTab {
    Mentions,
    Unreads,
}

impl Emerald {
    pub(in super::super) fn toggle_inbox(&mut self, cx: &mut Context<Self>) {
        if self.panel_is(PanelKind::Inbox) {
            return self.close_panel(cx);
        }
        self.panel = Some(Panel::Inbox {
            tab: InboxTab::Mentions,
            mentions: Results::loading(),
        });
        self.net.post(Command::Mentions { before: None });
        cx.notify();
    }

    fn inbox_tab(&mut self, tab: InboxTab, cx: &mut Context<Self>) {
        if let Some(Panel::Inbox { tab: shown, .. }) = &mut self.panel {
            *shown = tab;
            cx.notify();
        }
    }

    fn more_mentions(&mut self, cx: &mut Context<Self>) {
        let Some(Panel::Inbox { mentions, .. }) = &mut self.panel else { return };
        if mentions.loading {
            return;
        }
        mentions.loading = true;
        let before = mentions.items.last().map(|found| found.msg.id);
        self.net.post(Command::Mentions { before });
        cx.notify();
    }

    pub(in super::super) fn on_mentions(&mut self, before: Option<Snowflake>, messages: Result<Vec<Message>, String>) {
        let Some(Panel::Inbox { mentions, .. }) = &mut self.panel else { return };
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

    pub(super) fn render_inbox(&self, tab: InboxTab, mentions: &Results, cx: &mut Context<Self>) -> AnyElement {
        let tab_button = |id: &'static str, label: &'static str, this_tab: InboxTab, cx: &mut Context<Self>| {
            let on = tab == this_tab;
            div()
                .id(id)
                .px_2()
                .py_1()
                .rounded(theme::radius::control())
                .cursor_pointer()
                .text_size(zoomed(14.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(if on { theme::text_strong() } else { theme::interactive() }))
                .when(on, |b| b.bg(theme::selected()))
                .hover(|s| s.bg(theme::hover()))
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| this.inbox_tab(this_tab, cx)))
        };
        let body = match tab {
            InboxTab::Mentions => self.render_results("mentions", mentions, t!("inbox.no_mentions"), true, Self::more_mentions, cx),
            InboxTab::Unreads => self.render_unreads(cx),
        };
        div()
            .flex_1()
            .min_h(zoomed(0.))
            .flex()
            .flex_col()
            .child(
                div()
                    .px_3()
                    .py_2()
                    .flex()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme::border())
                    .child(tab_button("inbox-mentions", t!("inbox.mentions"), InboxTab::Mentions, cx))
                    .child(tab_button("inbox-unreads", t!("inbox.unreads"), InboxTab::Unreads, cx)),
            )
            .child(body)
            .into_any_element()
    }

    /// Unread channels outside muted places, servers in list order.
    fn unread_channels(&self) -> Vec<(Option<Snowflake>, Snowflake)> {
        let now = crate::time::now_secs();
        self.spaces
            .iter()
            .flat_map(|space| {
                let guild = (!space.is_dms()).then_some(space.id);
                space
                    .readable()
                    .filter(|c| self.reads.is_unread(c.id))
                    .filter(move |c| !self.notify.muted(&self.place_of(guild, c.id), now))
                    .map(move |c| (guild, c.id))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn render_unreads(&self, cx: &mut Context<Self>) -> AnyElement {
        let unread = self.unread_channels();
        if unread.is_empty() {
            return note(t!("inbox.no_unreads")).into_any_element();
        }
        div()
            .id("unreads")
            .flex_1()
            .min_h(zoomed(0.))
            .overflow_y_scroll()
            .p_2()
            .flex()
            .flex_col()
            .gap_1()
            .children(unread.into_iter().enumerate().map(|(ix, (guild, channel))| {
                let mentions = self.reads.mentions(channel);
                div()
                    .id(("unread", ix))
                    .h(zoomed(40.))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded(theme::radius::control())
                    .cursor_pointer()
                    .hover(|s| s.bg(theme::hover()))
                    .on_click(cx.listener(move |this, _, window, cx| this.open_channel_link(guild, channel, None, window, cx)))
                    .child(
                        div()
                            .flex_1()
                            .min_w(zoomed(0.))
                            .truncate()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::text_strong()))
                            .child(self.place_label(guild, channel)),
                    )
                    .when(mentions > 0, |row| row.child(widgets::badge(mentions as usize)))
                    .child(
                        div()
                            .id(("unread-read", ix))
                            .child(icons::icon(icons::CHECK, 18., theme::interactive()))
                            .tooltip(widgets::tooltip(t!("read.mark_read")))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.mark_channel_read(channel, cx);
                            })),
                    )
            }))
            .into_any_element()
    }
}
