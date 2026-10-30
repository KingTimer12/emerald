//! Panels beside the chat, opened from the header as in the web client:
//! pinned messages, search results, the inbox and a channel's threads.
//! One is open at a time, in place of the member list or profile.

use crate::zoom::zoomed;

mod found;
mod inbox;
mod pins;
mod search;
mod threads;

pub(super) use inbox::InboxTab;
pub(super) use search::SearchPanel;
pub(super) use threads::ThreadsPanel;

use gpui::{AnyElement, Context, FontWeight, SharedString, div, prelude::*, rgb};

use super::super::{icons, theme};
use super::state::Msg;
use super::{Emerald, widgets};
use emerald_discord::model::Snowflake;

const WIDTH: f32 = 420.;

/// A message shown in a panel, with where it lives.
pub(super) struct Found {
    pub msg: Msg,
    pub channel: Snowflake,
    pub guild: Option<Snowflake>,
}

/// A list of messages that loads page by page.
#[derive(Default)]
pub(super) struct Results {
    pub items: Vec<Found>,
    pub loading: bool,
    pub more: bool,
    pub error: Option<SharedString>,
}

impl Results {
    fn loading() -> Self {
        Self {
            loading: true,
            ..Self::default()
        }
    }
}

pub(super) enum Panel {
    Pins {
        channel: Snowflake,
        results: Results,
        /// When the last pin shown was pinned, to page from.
        last_pinned: Option<String>,
    },
    Search(SearchPanel),
    Inbox {
        tab: InboxTab,
        mentions: Results,
    },
    Threads(ThreadsPanel),
}

/// Which panel a header button opens, to tell whether it is the one open.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PanelKind {
    Pins,
    Search,
    Inbox,
    Threads,
}

impl Panel {
    fn kind(&self) -> PanelKind {
        match self {
            Self::Pins { .. } => PanelKind::Pins,
            Self::Search(_) => PanelKind::Search,
            Self::Inbox { .. } => PanelKind::Inbox,
            Self::Threads(_) => PanelKind::Threads,
        }
    }
}

impl Emerald {
    pub(super) fn panel_is(&self, kind: PanelKind) -> bool {
        self.panel.as_ref().is_some_and(|p| p.kind() == kind)
    }

    pub(super) fn close_panel(&mut self, cx: &mut Context<Self>) {
        if self.panel.take().is_some() {
            cx.notify();
        }
    }

    /// The panel opened from the channel header for another channel goes
    /// away with it; the inbox and a server search stay.
    pub(super) fn panel_follow(&mut self, channel: Snowflake) {
        let stale = match &self.panel {
            Some(Panel::Pins { channel: shown, .. }) => *shown != channel,
            Some(Panel::Threads(threads)) => threads.channel != channel,
            Some(Panel::Search(search)) => !search.covers(channel, self.spaces.guild_id_of(channel)),
            _ => false,
        };
        if stale {
            self.panel = None;
        }
    }

    /// The buttons at the right of the channel header: threads, pins, the
    /// search box and the inbox.
    pub(super) fn render_header_tools(&self, channel: Option<Snowflake>, cx: &mut Context<Self>) -> impl IntoElement {
        let space = self.spaces.current();
        let kind = channel.and_then(|id| space.channels().iter().find(|c| c.id == id)).map(|c| c.kind);
        let threads = !space.is_dms() && kind.is_some_and(|k| !k.is_thread() && !k.is_forum() && !k.is_voice());
        let pins = kind.is_some_and(|k| !k.is_forum()) || (space.is_dms() && channel.is_some());
        div()
            .flex()
            .items_center()
            .gap_2()
            .when(threads, |tools| {
                tools.child(tool(
                    "tool-threads",
                    icons::THREAD,
                    t!("threads.title"),
                    self.panel_is(PanelKind::Threads),
                    cx.listener(move |this, _, _, cx| {
                        if let Some(channel) = channel {
                            this.toggle_threads(channel, cx)
                        }
                    }),
                ))
            })
            .when(pins, |tools| {
                tools.child(tool(
                    "tool-pins",
                    icons::PIN,
                    t!("pins.title"),
                    self.panel_is(PanelKind::Pins),
                    cx.listener(move |this, _, _, cx| {
                        if let Some(channel) = channel {
                            this.toggle_pins(channel, cx)
                        }
                    }),
                ))
            })
            .when(channel.is_some(), |tools| {
                tools.child(
                    div()
                        .w(zoomed(220.))
                        .h(zoomed(28.))
                        .px_2()
                        .flex()
                        .items_center()
                        .gap_1()
                        .rounded(theme::radius::control())
                        .bg(rgb(theme::base_lowest()))
                        .child(icons::icon(icons::SEARCH, 16., theme::interactive()))
                        .child(div().flex_1().min_w(zoomed(0.)).child(self.search_input.clone())),
                )
            })
    }

    /// The open panel, beside the chat.
    pub(super) fn render_panel(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let panel = self.panel.as_ref()?;
        let (title, body) = match panel {
            Panel::Pins { results, .. } => (t!("pins.title").into(), self.render_pins(results, cx)),
            Panel::Search(search) => (self.search_title(search), self.render_search(search, cx)),
            Panel::Inbox { tab, mentions } => (t!("inbox.title").into(), self.render_inbox(*tab, mentions, cx)),
            Panel::Threads(threads) => (t!("threads.title").into(), self.render_threads(threads, cx)),
        };
        Some(
            div()
                .w(zoomed(WIDTH))
                .flex_none()
                .h_full()
                .flex()
                .flex_col()
                .border_l_1()
                .border_color(theme::border())
                .bg(rgb(theme::base_lower()))
                .child(
                    div()
                        .h(zoomed(48.))
                        .flex_none()
                        .px_4()
                        .flex()
                        .items_center()
                        .gap_2()
                        .border_b_1()
                        .border_color(theme::border())
                        .child(
                            div()
                                .flex_1()
                                .min_w(zoomed(0.))
                                .truncate()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(theme::text_strong()))
                                .child(title),
                        )
                        .child(
                            widgets::svg_button("panel-close", icons::CLOSE)
                                .on_click(cx.listener(|this, _, _, cx| this.close_panel(cx))),
                        ),
                )
                .child(body)
                .into_any_element(),
        )
    }
}

/// A header button that opens a panel, lit while it is open.
fn tool(
    id: &'static str,
    glyph: &'static str,
    tip: &'static str,
    open: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .size(zoomed(32.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme::radius::control())
        .cursor_pointer()
        .hover(|s| s.bg(theme::hover()))
        .tooltip(widgets::tooltip(tip))
        .child(icons::icon(glyph, 24., if open { theme::text_strong() } else { theme::interactive() }))
        .on_click(on_click)
}

/// A muted line in the middle of a panel: empty, loading or an error.
fn note(text: impl Into<SharedString>) -> gpui::Div {
    div()
        .p_4()
        .flex()
        .justify_center()
        .text_size(zoomed(14.))
        .text_color(theme::muted())
        .child(text.into())
}
