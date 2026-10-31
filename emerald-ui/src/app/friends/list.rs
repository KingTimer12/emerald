//! The friend list: rows with status and the buttons each relationship allows.

use super::*;
use super::super::super::icons;
use super::super::state::friend_filter::friend_matches;
use crate::zoom::zoomed;

impl Emerald {
    pub(super) fn render_friend_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let query = self.friends.search.read(cx).text().to_owned();
        let entries: Vec<Entry> = self
            .friend_entries()
            .into_iter()
            .filter(|entry| friend_matches(&query, &entry.name, &entry.username))
            .collect();
        let (title, empty) = match self.friends.tab {
            FriendsTab::Online => (t!("friends.online_header"), t!("friends.empty_online")),
            FriendsTab::All => (t!("friends.all_header"), t!("friends.empty_all")),
            FriendsTab::Blocked => (t!("friends.blocked_header"), t!("friends.empty_blocked")),
            _ => (t!("friends.pending_header"), t!("friends.empty_pending")),
        };
        let search = div()
            .mx(zoomed(30.))
            .mt_4()
            .mb_2()
            .h(zoomed(40.))
            .px_3()
            .flex()
            .items_center()
            .gap_2()
            .rounded(theme::radius::control())
            .border_1()
            .border_color(theme::border_normal())
            .bg(rgb(theme::surface_high()))
            .child(icons::icon(icons::SEARCH, 18., theme::interactive()))
            .child(div().flex_1().min_w(zoomed(0.)).child(self.friends.search.clone()));
        if entries.is_empty() {
            return div()
                .flex_1()
                .flex()
                .flex_col()
                .child(search)
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .px_8()
                        .text_color(theme::muted())
                        .child(empty),
                )
                .into_any_element();
        }

        let count = entries.len();
        let mut rows = Vec::with_capacity(count);
        for entry in entries {
            rows.push(self.render_friend(entry, cx));
        }
        div()
            .flex_1()
            .min_h(zoomed(0.))
            .flex()
            .flex_col()
            .child(search)
            .child(
                div()
                    .id("friend-list")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_y_scroll()
                    .pl(zoomed(30.))
                    .pr(zoomed(20.))
                    .child(
                        div()
                            .mb_2()
                            .text_size(zoomed(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(theme::text_muted()))
                            .child(format!("{title} \u{2014} {count}")),
                    )
                    .children(rows),
            )
            .into_any_element()
    }

    pub(super) fn render_friend(&self, entry: Entry, cx: &mut Context<Self>) -> AnyElement {
        let id = entry.id;
        let row_group: SharedString = format!("friend-{}", id.0).into();
        let note = (entry.kind == relationship_kind::INCOMING)
            .then(|| self.people.request_note(id))
            .flatten()
            .map(str::to_owned);
        let note_shown = self.friends.shown_note == Some(id);
        let subtitle: SharedString = match entry.kind {
            relationship_kind::INCOMING => t!("friends.incoming").into(),
            relationship_kind::OUTGOING => t!("friends.outgoing").into(),
            relationship_kind::BLOCKED => t!("friends.blocked").into(),
            _ => match super::super::activity::activity_text(self.people.activities(id)) {
                Some(activity) => activity.into(),
                None => widgets::status_label(entry.status.as_ref().map(|s| -> &str { s.as_ref() })).into(),
            },
        };
        let blocked = entry.kind == relationship_kind::BLOCKED;
        let mut actions = div().flex().flex_none().gap_2();
        actions = match entry.kind {
            relationship_kind::BLOCKED => actions.child(
                round(("unblock", id.0 as usize), icons::CLOSE, t!("user_menu.unblock")).on_click(
                    cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.net.post(Command::Block { user: id, block: false });
                    }),
                ),
            ),
            relationship_kind::INCOMING => actions
                .when(note.is_some() && !note_shown, |actions| {
                    actions.child(
                        round(("view-request", id.0 as usize), icons::EYE, t!("friends.view_request"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.friends.shown_note = Some(id);
                                cx.notify();
                            })),
                    )
                })
                .child(
                    round(("accept", id.0 as usize), icons::CHECK, t!("friends.accept")).on_click(
                        cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.net.post(Command::AcceptFriend { user: id });
                        }),
                    ),
                )
                .child(
                    round(("decline", id.0 as usize), icons::CLOSE, t!("friends.decline")).on_click(
                        cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.net.post(Command::RemoveRelationship { user: id });
                        }),
                    ),
                ),
            relationship_kind::OUTGOING => actions.child(
                round(("cancel", id.0 as usize), icons::CLOSE, t!("common.cancel")).on_click(
                    cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.net.post(Command::RemoveRelationship { user: id });
                    }),
                ),
            ),
            // Message, and the user menu (which holds "Remove Friend").
            _ => actions
                .child(
                    round(("message", id.0 as usize), icons::CHAT_BUBBLE, t!("friends.message"))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            cx.stop_propagation();
                            this.open_dm(id, window, cx);
                        })),
                )
                .child({
                    let tracked = widgets::popover::Tracked::default();
                    tracked.track(round(("more", id.0 as usize), icons::MORE, t!("friends.more"))).on_click(
                        cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            let anchor = widgets::popover::Anchor::below_end(tracked.bounds());
                            this.open_user_menu(id, None, None, anchor, cx);
                        }),
                    )
                }),
        };

        div()
            .id(("friend", id.0 as usize))
            .group(row_group.clone())
            .min_h(zoomed(62.))
            .py_2()
            .flex_none()
            .flex()
            .items_center()
            .gap_3()
            .px_2()
            .border_t_1()
            .border_color(theme::border())
            .rounded(theme::radius::control())
            .cursor_pointer()
            .hover(|s| s.bg(theme::hover()))
            .child(widgets::avatar(
                &entry.name,
                id,
                self.user_avatar(id),
                32.,
                entry.status.as_ref().map(|s| -> (&str, u32) { (s.as_ref(), theme::base_lower()) }).or(
                    (entry.kind == relationship_kind::FRIEND).then_some(("offline", theme::base_lower())),
                ),
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(zoomed(0.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .truncate()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(theme::text_strong()))
                                    .child(entry.name),
                            )
                            // The username shows only while the row is hovered, as in 2026.
                            .child(
                                div()
                                    .truncate()
                                    .text_size(zoomed(14.))
                                    .text_color(rgb(theme::text_muted()))
                                    .opacity(0.)
                                    .group_hover(row_group.clone(), |s| s.opacity(1.))
                                    .child(entry.username),
                            ),
                    )
                    .child(
                        div()
                            .text_size(zoomed(14.))
                            .text_color(rgb(theme::text_muted()))
                            .child(subtitle),
                    )
                    .when_some(note.filter(|_| note_shown), |column, note| {
                        column.child(
                            div()
                                .mt_1()
                                .text_size(zoomed(14.))
                                .text_color(rgb(theme::text_default()))
                                .child(note),
                        )
                    }),
            )
            .child(actions)
            .on_click(cx.listener(move |this, _, window, cx| {
                if !blocked {
                    this.open_dm(id, window, cx);
                }
            }))
            .on_mouse_down(gpui::MouseButton::Right, self.on_user_right_click(id, None, None, cx))
            .into_any_element()
    }
}

/// A 36px round icon button on a friend row; its icon brightens on hover.
fn round(id: (&'static str, usize), glyph: &'static str, tip: &'static str) -> gpui::Stateful<gpui::Div> {
    let group: SharedString = format!("{}-{}", id.0, id.1).into();
    div()
        .id(id)
        .group(group.clone())
        .size(zoomed(36.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(rgb(theme::base_low()))
        .cursor_pointer()
        .tooltip(widgets::tooltip(tip))
        .child(
            icons::icon(glyph, 20., theme::interactive())
                .group_hover(group, |s| s.text_color(rgb(theme::text_strong()))),
        )
}
