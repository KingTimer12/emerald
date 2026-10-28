//! Friends list: the DM home screen with Online / All / Pending tabs and the
//! "Add Friend" form, worded like the web client.

use crate::zoom::zoomed;

mod list;

use gpui::{AnyElement, Context, Entity, Focusable, FontWeight, SharedString, div, prelude::*, rgb};

use super::super::text_input::TextInput;
use super::super::theme;
use super::widgets::{self, ButtonKind};
use super::{Emerald, Feedback};
use emerald_discord::model::{Snowflake, relationship_kind};
use emerald_discord::{Action, Command};

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
    Blocked,
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
                FriendsTab::Blocked => *kind == relationship_kind::BLOCKED,
                FriendsTab::Add => false,
            })
            .map(|(id, kind)| {
                let user = self.people.user(id);
                Entry {
                    id,
                    name: self
                        .people
                        .friend_name(id)
                        .map_or_else(|| id.to_string(), str::to_owned)
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
                .py(zoomed(2.))
                .rounded_md()
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .text_color(rgb(if selected { theme::text_strong() } else { theme::interactive() }))
                .when(selected, |t| t.bg(theme::selected()))
                .when(!selected, |t| {
                    t.hover(|s| s.bg(theme::hover()).text_color(rgb(theme::text_strong())))
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
            .h(zoomed(48.))
            .flex_none()
            .flex()
            .items_center()
            .gap_4()
            .px_4()
            .border_b_1()
            .border_color(theme::border())
            .child(
                div()
                    .text_color(rgb(theme::text_strong()))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(t!("friends.title")),
            )
            .child(div().w(zoomed(1.)).h(zoomed(24.)).bg(theme::selected()))
            .child(tab("tab-online", t!("friends.tab_online"), FriendsTab::Online, cx))
            .child(tab("tab-all", t!("friends.tab_all"), FriendsTab::All, cx))
            .child(
                tab("tab-pending", t!("friends.tab_pending"), FriendsTab::Pending, cx)
                    .when(pending > 0, |t| t.child(widgets::badge(pending))),
            )
            .child(tab("tab-blocked", t!("friends.tab_blocked"), FriendsTab::Blocked, cx))
            .child(
                // Green call-to-action that turns into green text when open.
                div()
                    .id("tab-add")
                    .px_2()
                    .py(zoomed(2.))
                    .rounded_md()
                    .font_weight(FontWeight::MEDIUM)
                    .cursor_pointer()
                    .when(adding, |t| t.text_color(rgb(theme::online())))
                    .when(!adding, |t| {
                        t.bg(rgb(theme::button_positive()))
                            .text_color(rgb(theme::text_strong()))
                            .hover(|s| s.bg(rgb(theme::button_positive_hover())))
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
            .min_w(zoomed(0.))
            .h_full()
            .bg(rgb(theme::base_lower()))
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
            .px(zoomed(30.))
            .py_5()
            .border_b_1()
            .border_color(theme::border())
            .child(
                div()
                    .text_size(zoomed(20.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::text_strong()))
                    .child(t!("friends.add")),
            )
            .child(
                div()
                    .text_size(zoomed(14.))
                    .text_color(rgb(theme::text_muted()))
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
}
