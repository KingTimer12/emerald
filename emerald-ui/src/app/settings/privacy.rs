//! Privacy & Safety: who can DM the user, message requests, and who can
//! send friend requests, saved in the synced settings.

use super::*;
use emerald_discord::SettingsEdit;
use emerald_discord::model::friend_source;

/// A default changed; the web client then asks whether to apply it to the
/// servers already joined too.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Ask {
    Dms { restricted: bool },
    MessageRequests { restricted: bool },
}

impl Emerald {
    fn ask_privacy(&mut self, ask: Ask, cx: &mut Context<Self>) {
        if let Some(view) = self.settings_view() {
            view.privacy_ask = Some(ask);
        }
        cx.notify();
    }

    /// `existing`: also apply it to every server already joined.
    fn answer_privacy(&mut self, existing: bool, cx: &mut Context<Self>) {
        let Some(ask) = self.settings_view().and_then(|view| view.privacy_ask.take()) else { return };
        let list = |restricted: bool| existing.then(|| if restricted { self.joined_guilds() } else { Vec::new() });
        let edit = match ask {
            Ask::Dms { restricted } => SettingsEdit::DmsRestricted {
                restricted,
                existing: list(restricted),
            },
            Ask::MessageRequests { restricted } => SettingsEdit::MessageRequestsRestricted {
                restricted,
                existing: list(restricted),
            },
        };
        self.edit_privacy(edit, cx);
    }

    pub(super) fn render_privacy(&self, view: &SettingsView, cx: &mut Context<Self>) -> AnyElement {
        let Some(privacy) = self.account.privacy() else {
            return div().child(page_title(t!("settings.privacy"))).child(hint(t!("privacy.loading"))).into_any_element();
        };
        let busy = self.busy.contains(&Action::AccountSettings);
        let flags = privacy.friend_sources;
        let everyone = flags & friend_source::EVERYONE == friend_source::EVERYONE;
        let flip = move |bit: u32| move |this: &mut Self, _: &gpui::ClickEvent, _: &mut Window, cx: &mut Context<Self>| {
            if !busy {
                this.edit_privacy(SettingsEdit::FriendSources(flags ^ bit), cx);
            }
        };

        div()
            .flex()
            .flex_col()
            .child(page_title(t!("settings.privacy")))
            .child(section_title(t!("privacy.server_section")))
            .child(
                switch_row("dms-allowed", t!("privacy.dms"), t!("privacy.dms_hint"), !privacy.dms_restricted, !busy)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !busy {
                            this.ask_privacy(Ask::Dms { restricted: !privacy.dms_restricted }, cx);
                        }
                    })),
            )
            .child({
                let enabled = !busy && !privacy.dms_restricted;
                let on = !privacy.dms_restricted && !privacy.message_requests_restricted;
                switch_row("message-requests", t!("privacy.requests"), t!("privacy.requests_hint"), on, enabled).on_click(
                    cx.listener(move |this, _, _, cx| {
                        if enabled {
                            this.ask_privacy(Ask::MessageRequests { restricted: on }, cx);
                        }
                    }),
                )
            })
            .children(view.privacy_ask.map(|_| self.render_privacy_ask(cx)))
            .child(section_title(t!("privacy.friends_section")).mt_10())
            .child(hint(t!("privacy.friends_hint")).mb_2())
            .child(
                switch_row("friends-everyone", t!("privacy.everyone"), t!("privacy.everyone_hint"), everyone, !busy).on_click(
                    cx.listener(move |this, _, _, cx| {
                        if !busy {
                            let flags = if everyone {
                                friend_source::MUTUAL_FRIENDS | friend_source::MUTUAL_GUILDS
                            } else {
                                friend_source::EVERYONE
                            };
                            this.edit_privacy(SettingsEdit::FriendSources(flags), cx);
                        }
                    }),
                ),
            )
            .child(
                switch_row(
                    "friends-of-friends",
                    t!("privacy.friends_of_friends"),
                    t!("privacy.friends_of_friends_hint"),
                    flags & friend_source::MUTUAL_FRIENDS != 0,
                    !busy && !everyone,
                )
                .when(!everyone, |row| row.on_click(cx.listener(flip(friend_source::MUTUAL_FRIENDS)))),
            )
            .child(
                switch_row(
                    "friends-server-members",
                    t!("privacy.server_members"),
                    t!("privacy.server_members_hint"),
                    flags & friend_source::MUTUAL_GUILDS != 0,
                    !busy && !everyone,
                )
                .when(!everyone, |row| row.on_click(cx.listener(flip(friend_source::MUTUAL_GUILDS)))),
            )
            .children(view.feedback.as_ref().map(|f| widgets::feedback(f).mt_4()))
            .into_any_element()
    }

    fn render_privacy_ask(&self, cx: &mut Context<Self>) -> gpui::Div {
        div()
            .my_3()
            .p_4()
            .rounded_lg()
            .bg(rgb(theme::base_lowest()))
            .flex()
            .flex_col()
            .gap_3()
            .child(section_title(t!("privacy.apply_title")))
            .child(hint(t!("privacy.apply_hint")))
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        widgets::button("privacy-apply-all", t!("privacy.apply_all"), ButtonKind::Brand)
                            .on_click(cx.listener(|this, _, _, cx| this.answer_privacy(true, cx))),
                    )
                    .child(
                        widgets::button("privacy-apply-new", t!("privacy.apply_new"), ButtonKind::Secondary)
                            .on_click(cx.listener(|this, _, _, cx| this.answer_privacy(false, cx))),
                    ),
            )
    }
}
