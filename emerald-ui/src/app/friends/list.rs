//! The friend list: rows with status and the buttons each relationship allows.

use super::*;

impl Emerald {
    pub(super) fn render_friend_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let entries = self.friend_entries();
        let (title, empty) = match self.friends.tab {
            FriendsTab::Online => (
                t!("friends.online_header"),
                t!("friends.empty_online"),
            ),
            FriendsTab::All => (
                t!("friends.all_header"),
                t!("friends.empty_all"),
            ),
            FriendsTab::Blocked => (
                t!("friends.blocked_header"),
                t!("friends.empty_blocked"),
            ),
            _ => (
                t!("friends.pending_header"),
                t!("friends.empty_pending"),
            ),
        };
        if entries.is_empty() {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .px_8()
                .text_color(theme::muted())
                .child(empty)
                .into_any_element();
        }

        let count = entries.len();
        let mut rows = Vec::with_capacity(count);
        for entry in entries {
            rows.push(self.render_friend(entry, cx));
        }
        div()
            .id("friend-list")
            .flex_1()
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .pl(px(30.))
            .pr(px(20.))
            .pt_4()
            .child(widgets::label(format!("{title} — {count}")).mb_2())
            .children(rows)
            .into_any_element()
    }

    pub(super) fn render_friend(&self, entry: Entry, cx: &mut Context<Self>) -> AnyElement {
        let id = entry.id;
        let subtitle: SharedString = match entry.kind {
            relationship_kind::INCOMING => t!("friends.incoming").into(),
            relationship_kind::OUTGOING => t!("friends.outgoing").into(),
            relationship_kind::BLOCKED => t!("friends.blocked").into(),
            _ => widgets::status_label(entry.status.as_ref().map(|s| -> &str { s.as_ref() })).into(),
        };
        let small = |button: gpui::Stateful<gpui::Div>| button.h(px(32.)).px_3();
        // Discord lets you message anyone you can reach, pending or not.
        let message = small(widgets::button(("message", id.0 as usize), t!("friends.message"), ButtonKind::Secondary))
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                this.open_dm(id, window, cx);
            }));

        let blocked = entry.kind == relationship_kind::BLOCKED;
        let actions = div().flex().flex_none().gap_2().when(!blocked, |a| a.child(message));
        let actions = match entry.kind {
            relationship_kind::BLOCKED => actions.child(
                small(widgets::button(("unblock", id.0 as usize), t!("user_menu.unblock"), ButtonKind::Secondary))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.net.post(Command::Block { user: id, block: false });
                    })),
            ),
            relationship_kind::INCOMING => actions
                .child(
                    small(widgets::button(("accept", id.0 as usize), t!("friends.accept"), ButtonKind::Positive))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.net.post(Command::AcceptFriend { user: id });
                        })),
                )
                .child(
                    small(widgets::button(("decline", id.0 as usize), t!("friends.decline"), ButtonKind::Secondary))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.net.post(Command::RemoveRelationship { user: id });
                        })),
                ),
            relationship_kind::OUTGOING => actions.child(
                small(widgets::button(("cancel", id.0 as usize), t!("common.cancel"), ButtonKind::Secondary))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.net.post(Command::RemoveRelationship { user: id });
                    })),
            ),
            _ => {
                let confirming = self.friends.confirm_remove == Some(id);
                actions
                    .child(
                        // Removing a friend is a two-click action.
                        small(widgets::button(
                            ("remove", id.0 as usize),
                            if confirming { t!("friends.confirm_remove") } else { t!("friends.remove") },
                            if confirming { ButtonKind::Danger } else { ButtonKind::Secondary },
                        ))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            if this.friends.confirm_remove == Some(id) {
                                this.friends.confirm_remove = None;
                                this.net.post(Command::RemoveRelationship { user: id });
                            } else {
                                this.friends.confirm_remove = Some(id);
                            }
                            cx.notify();
                        })),
                    )
            }
        };

        div()
            .id(("friend", id.0 as usize))
            .h(px(62.))
            .flex_none()
            .flex()
            .items_center()
            .gap_3()
            .px_2()
            .border_t_1()
            .border_color(theme::border())
            .rounded_lg()
            .cursor_pointer()
            .hover(|s| s.bg(theme::hover()))
            .child(widgets::avatar(
                &entry.name,
                id,
                self.user_avatar(id),
                32.,
                entry.status.as_ref().map(|s| -> (&str, u32) { (s.as_ref(), theme::BASE_LOWER) }).or(
                    (entry.kind == relationship_kind::FRIEND).then_some(("offline", theme::BASE_LOWER)),
                ),
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(0.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .truncate()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(theme::TEXT_STRONG))
                                    .child(entry.name),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(14.))
                                    .text_color(rgb(theme::TEXT_MUTED))
                                    .child(entry.username),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(14.))
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child(subtitle),
                    ),
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
