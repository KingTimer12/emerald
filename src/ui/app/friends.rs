//! Friends list: the DM home screen with Online / All / Pending tabs and the
//! "Add Friend" form, worded like the web client.

use gpui::{AnyElement, Context, Entity, Focusable, FontWeight, SharedString, div, prelude::*, px, rgb};

use super::super::text_input::TextInput;
use super::super::theme;
use super::widgets::{self, ButtonKind};
use super::{Emerald, Feedback};
use crate::discord::model::{Snowflake, relationship_kind};
use crate::discord::{Action, Command};

/// State of the friends screen.
pub(super) struct Friends {
    pub tab: FriendsTab,
    pub input: Entity<TextInput>,
    pub feedback: Option<Feedback>,
    /// Username of the request in flight, for the success message.
    pub pending: Option<String>,
    /// Friend whose "Remove" was clicked once and awaits confirmation.
    pub confirm_remove: Option<Snowflake>,
}

impl Friends {
    pub fn new(cx: &mut Context<Emerald>) -> Self {
        Self {
            tab: FriendsTab::Online,
            input: cx.new(|cx| TextInput::new(t!("friends.username_placeholder"), cx).keep_on_submit()),
            feedback: None,
            pending: None,
            confirm_remove: None,
        }
    }

    /// The relationship with `user` ended.
    pub fn forget(&mut self, user: Snowflake) {
        if self.confirm_remove == Some(user) {
            self.confirm_remove = None;
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FriendsTab {
    Online,
    All,
    Pending,
    Add,
}

struct Entry {
    id: Snowflake,
    name: SharedString,
    username: SharedString,
    kind: u8,
    status: Option<SharedString>,
}

impl Emerald {
    fn friend_entries(&self) -> Vec<Entry> {
        let mut entries: Vec<Entry> = self
            .people
            .relationships()
            .filter(|(id, kind)| match self.friends.tab {
                FriendsTab::Online => {
                    *kind == relationship_kind::FRIEND && self.people.status(*id).is_some()
                }
                FriendsTab::All => *kind == relationship_kind::FRIEND,
                FriendsTab::Pending => {
                    matches!(*kind, relationship_kind::INCOMING | relationship_kind::OUTGOING)
                }
                FriendsTab::Add => false,
            })
            .map(|(id, kind)| {
                let user = self.people.user(id);
                Entry {
                    id,
                    name: user
                        .map_or_else(|| id.to_string(), |u| u.display_name().to_owned())
                        .into(),
                    username: user.map(|u| u.username.clone()).unwrap_or_default().into(),
                    kind,
                    status: self.people.status(id).cloned(),
                }
            })
            .collect();
        entries.sort_by_cached_key(|e| e.name.to_lowercase());
        entries
    }

    pub(super) fn send_friend_request(&mut self, username: String, cx: &mut Context<Self>) {
        let username = username.trim().trim_start_matches('@').to_owned();
        if username.is_empty() || self.busy.contains(&Action::FriendRequest) {
            return;
        }
        self.friends.feedback = None;
        self.friends.pending = Some(username.clone());
        self.run(Action::FriendRequest, Command::SendFriendRequest { username }, cx);
    }

    pub(super) fn render_friends(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = |id: &'static str, label: &'static str, tab: FriendsTab, cx: &mut Context<Self>| {
            let selected = self.friends.tab == tab;
            div()
                .id(id)
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .py(px(2.))
                .rounded_md()
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .text_color(rgb(if selected { theme::TEXT_STRONG } else { theme::INTERACTIVE }))
                .when(selected, |t| t.bg(theme::selected()))
                .when(!selected, |t| {
                    t.hover(|s| s.bg(theme::hover()).text_color(rgb(theme::TEXT_STRONG)))
                })
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.friends.tab = tab;
                    this.friends.confirm_remove = None;
                    cx.notify();
                }))
        };
        let pending = self.people.incoming_requests();
        let adding = self.friends.tab == FriendsTab::Add;

        let header = div()
            .h(px(48.))
            .flex_none()
            .flex()
            .items_center()
            .gap_4()
            .px_4()
            .border_b_1()
            .border_color(theme::border())
            .child(
                div()
                    .text_color(rgb(theme::TEXT_STRONG))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(t!("friends.title")),
            )
            .child(div().w(px(1.)).h(px(24.)).bg(theme::selected()))
            .child(tab("tab-online", t!("friends.tab_online"), FriendsTab::Online, cx))
            .child(tab("tab-all", t!("friends.tab_all"), FriendsTab::All, cx))
            .child(
                tab("tab-pending", t!("friends.tab_pending"), FriendsTab::Pending, cx)
                    .when(pending > 0, |t| t.child(widgets::badge(pending))),
            )
            .child(
                // Green call-to-action that turns into green text when open.
                div()
                    .id("tab-add")
                    .px_2()
                    .py(px(2.))
                    .rounded_md()
                    .font_weight(FontWeight::MEDIUM)
                    .cursor_pointer()
                    .when(adding, |t| t.text_color(rgb(theme::ONLINE)))
                    .when(!adding, |t| {
                        t.bg(rgb(theme::BUTTON_POSITIVE))
                            .text_color(rgb(theme::TEXT_STRONG))
                            .hover(|s| s.bg(rgb(theme::BUTTON_POSITIVE_HOVER)))
                    })
                    .child(t!("friends.add"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.friends.tab = FriendsTab::Add;
                        window.focus(&this.friends.input.focus_handle(cx));
                        cx.notify();
                    })),
            );

        let body = if adding {
            self.render_add_friend(cx).into_any_element()
        } else {
            self.render_friend_list(cx)
        };

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .bg(rgb(theme::BASE_LOWER))
            .border_l_1()
            .border_color(theme::border())
            .child(header)
            .child(body)
    }

    fn render_add_friend(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = self.busy.contains(&Action::FriendRequest);
        let empty = self.friends.input.read(cx).text().trim().is_empty();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .px(px(30.))
            .py_5()
            .border_b_1()
            .border_color(theme::border())
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT_STRONG))
                    .child(t!("friends.add")),
            )
            .child(
                div()
                    .text_size(px(14.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(t!("friends.add_hint")),
            )
            .child(
                div()
                    .mt_2()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(div().flex_1().child(self.friends.input.clone()))
                    .child(
                        widgets::button(
                            "send-friend-request",
                            if busy { t!("friends.sending") } else { t!("friends.send") },
                            ButtonKind::Brand,
                        )
                        .when(empty || busy, |b| b.opacity(0.5))
                        .on_click(cx.listener(|this, _, _, cx| {
                            let username = this.friends.input.read(cx).text().to_owned();
                            this.send_friend_request(username, cx);
                        })),
                    ),
            )
            .children(self.friends.feedback.as_ref().map(widgets::feedback))
    }

    fn render_friend_list(&self, cx: &mut Context<Self>) -> AnyElement {
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

    fn render_friend(&self, entry: Entry, cx: &mut Context<Self>) -> AnyElement {
        let id = entry.id;
        let subtitle: SharedString = match entry.kind {
            relationship_kind::INCOMING => t!("friends.incoming").into(),
            relationship_kind::OUTGOING => t!("friends.outgoing").into(),
            _ => widgets::status_label(entry.status.as_ref().map(|s| -> &str { s.as_ref() })).into(),
        };
        let small = |button: gpui::Stateful<gpui::Div>| button.h(px(32.)).px_3();
        // Discord lets you message anyone you can reach, pending or not.
        let message = small(widgets::button(("message", id.0 as usize), t!("friends.message"), ButtonKind::Secondary))
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                this.open_dm(id, window, cx);
            }));

        let actions = div().flex().flex_none().gap_2().child(message);
        let actions = match entry.kind {
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
                this.open_dm(id, window, cx);
            }))
            .into_any_element()
    }
}
