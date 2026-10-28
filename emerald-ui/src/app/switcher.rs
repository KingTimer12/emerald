//! The quick switcher (Ctrl/Cmd+K): type part of a channel, DM or server
//! name and jump there, as in the web client. Matching lives in
//! [`state::switcher`](super::state::switcher).

use gpui::{
    AnyElement, Context, Entity, Focusable, FontWeight, MouseButton, Subscription, Window, div, prelude::*, rgb,
};

use super::super::text_input::{MenuKey, TextInput};
use super::super::{icons, theme};
use super::state::switcher::{self, Candidate, Place};
use super::state::Row;
use super::{Emerald, Overlay, widgets};
use crate::zoom::zoomed;

pub struct Switcher {
    input: Entity<TextInput>,
    /// Every place, recent ones first.
    candidates: Vec<Candidate>,
    /// Indices into `candidates` matching the query, best first.
    results: Vec<usize>,
    selected: usize,
    _subscriptions: [Subscription; 2],
}

impl Emerald {
    pub(super) fn open_switcher(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.overlay, Some(Overlay::Switcher(_))) {
            return self.close_overlay(window, cx);
        }
        let input = cx.new(|cx| {
            let mut input = TextInput::new(t!("switcher.placeholder"), cx).keep_on_submit();
            // Arrows and Enter go to the result list.
            input.set_menu_open(true);
            input
        });
        let subscriptions = [
            cx.observe(&input, |this, _, cx| this.on_switcher_query(cx)),
            cx.subscribe_in(&input, window, |this, _, key: &MenuKey, window, cx| {
                this.on_switcher_key(*key, window, cx)
            }),
        ];
        let candidates = self.switcher_candidates();
        let results = switcher::rank("", &candidates);
        window.focus(&input.focus_handle(cx));
        self.overlay = Some(Overlay::Switcher(Switcher {
            input,
            candidates,
            results,
            selected: 0,
            _subscriptions: subscriptions,
        }));
        cx.notify();
    }

    /// Every channel, DM and server the user can open, recent channels
    /// first.
    fn switcher_candidates(&self) -> Vec<Candidate> {
        let mut candidates = Vec::new();
        for space in self.spaces.iter() {
            if !space.is_dms() {
                candidates.push(Candidate {
                    place: Place::Guild(space.id),
                    name: space.name.to_string(),
                    detail: None,
                    icon: "",
                    user: None,
                });
            }
            for row in space.rows() {
                let Row::Channel(row) = row else { continue };
                if !row.kind.is_openable() {
                    continue;
                }
                let place = if space.is_dms() {
                    Place::Dm(row.id)
                } else {
                    Place::Channel {
                        guild: space.id,
                        channel: row.id,
                    }
                };
                candidates.push(Candidate {
                    place,
                    name: row.name.to_string(),
                    detail: (!space.is_dms()).then(|| space.name.to_string()),
                    icon: row.icon,
                    user: row.user,
                });
            }
        }
        let recency = |c: &Candidate| match c.place {
            Place::Channel { channel, .. } | Place::Dm(channel) => self.recent.position(channel),
            Place::Guild(_) => None,
        };
        // Stable: places never visited keep the sidebar order.
        candidates.sort_by_key(|c| recency(c).unwrap_or(usize::MAX));
        candidates
    }

    fn on_switcher_query(&mut self, cx: &mut Context<Self>) {
        let Some(Overlay::Switcher(switcher)) = &mut self.overlay else { return };
        let query = switcher.input.read(cx).text().to_owned();
        switcher.results = switcher::rank(&query, &switcher.candidates);
        switcher.selected = 0;
        cx.notify();
    }

    fn on_switcher_key(&mut self, key: MenuKey, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Overlay::Switcher(switcher)) = &mut self.overlay else { return };
        let count = switcher.results.len();
        match key {
            MenuKey::Up if count > 0 => switcher.selected = (switcher.selected + count - 1) % count,
            MenuKey::Down if count > 0 => switcher.selected = (switcher.selected + 1) % count,
            MenuKey::Accept => {
                let selected = switcher.selected;
                return self.pick_switcher(selected, window, cx);
            }
            _ => {}
        }
        cx.notify();
    }

    /// Goes to result `ix` and closes the switcher.
    fn pick_switcher(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Overlay::Switcher(switcher)) = &self.overlay else { return };
        let Some(place) = switcher.results.get(ix).map(|c| switcher.candidates[*c].place) else { return };
        self.close_overlay(window, cx);
        match place {
            Place::Channel { guild, channel } => self.open_channel_link(Some(guild), channel, None, window, cx),
            Place::Dm(channel) => {
                self.spaces.select_dms();
                self.select_channel(channel, window, cx);
            }
            Place::Guild(guild) => {
                if let Some(ix) = self.spaces.index(guild) {
                    self.select_space(ix, window, cx);
                }
            }
        }
    }

    pub(super) fn render_switcher(&self, switcher: &Switcher, cx: &mut Context<Self>) -> AnyElement {
        let rows = switcher.results.iter().enumerate().map(|(ix, candidate)| {
            let candidate = &switcher.candidates[*candidate];
            let selected = ix == switcher.selected;
            let leading = match (candidate.user, candidate.place) {
                (Some(user), _) => {
                    widgets::avatar(&candidate.name, user, self.user_avatar(user), 20., None).into_any_element()
                }
                (None, Place::Guild(_)) => div()
                    .size(zoomed(20.))
                    .rounded(zoomed(6.))
                    .bg(rgb(theme::base_low()))
                    .into_any_element(),
                (None, _) => icons::icon(candidate.icon, 20., theme::channels_default()).into_any_element(),
            };
            div()
                .id(("switcher-row", ix))
                .h(zoomed(34.))
                .flex_none()
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .rounded_md()
                .cursor_pointer()
                .when(selected, |row| row.bg(theme::selected()))
                .hover(|s| s.bg(theme::hover()))
                .child(div().size(zoomed(20.)).flex_none().flex().items_center().justify_center().child(leading))
                .child(
                    div()
                        .flex_none()
                        .max_w(zoomed(320.))
                        .truncate()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(theme::text_strong()))
                        .child(candidate.name.clone()),
                )
                .children(candidate.detail.clone().map(|detail| {
                    div()
                        .flex_1()
                        .min_w(zoomed(0.))
                        .truncate()
                        .text_size(zoomed(12.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::muted())
                        .child(detail.to_uppercase())
                }))
                .on_click(cx.listener(move |this, _, window, cx| this.pick_switcher(ix, window, cx)))
                .into_any_element()
        });
        let empty = switcher.results.is_empty().then(|| {
            div()
                .p_4()
                .text_size(zoomed(14.))
                .text_color(theme::muted())
                .child(t!("switcher.empty"))
        });
        let card = div()
            .id("switcher")
            .w(zoomed(570.))
            .mt(zoomed(120.))
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(theme::border())
            .bg(rgb(theme::base_lower()))
            .shadow_lg()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .h(zoomed(56.))
                    .px_3()
                    .flex()
                    .items_center()
                    .rounded_md()
                    .bg(rgb(theme::base_lowest()))
                    .text_size(zoomed(18.))
                    .child(div().flex_1().child(switcher.input.clone())),
            )
            .child(
                div()
                    .id("switcher-results")
                    .max_h(zoomed(34. * 10.))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .children(rows)
                    .children(empty),
            )
            .child(
                div()
                    .text_size(zoomed(12.))
                    .text_color(theme::muted())
                    .child(t!("switcher.tip")),
            );
        div()
            .id("switcher-scrim")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .justify_center()
            .items_start()
            .bg(theme::scrim())
            .occlude()
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| this.close_overlay(window, cx)))
            .child(card)
            .into_any_element()
    }
}
