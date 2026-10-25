//! Invites: "Invite People", which makes a link to a channel, and the
//! cards drawn under messages that link an invite, with a Join button.

use std::cell::RefCell;
use std::collections::HashMap;

use gpui::{AnyElement, ClipboardItem, Context, FontWeight, MouseButton, SharedString, div, img, prelude::*, px, rgb};

use super::super::{markdown, theme};
use super::widgets::{self, ButtonKind};
use super::{Emerald, Overlay};
use emerald_discord::model::{Invite, Snowflake};
use emerald_discord::{Action, Command};

/// Cards under one message, at most.
const CARDS_PER_MESSAGE: usize = 3;
/// Invites resolved this session; past this many the oldest go.
const CARD_CACHE: usize = 200;

/// Invites resolved for their cards, by code; `None` while asked for or
/// when the invite is dead.
#[derive(Default)]
pub(super) struct InviteCards {
    known: RefCell<HashMap<String, Option<Option<Invite>>>>,
}

impl InviteCards {
    /// The invite behind `code`: `None` while unknown, which asks for it
    /// once, else whether it is alive.
    fn get(&self, code: &str, net: &dyn super::Outbox) -> Option<Option<Invite>> {
        let mut known = self.known.borrow_mut();
        if let Some(found) = known.get(code) {
            return found.clone();
        }
        if known.len() >= CARD_CACHE {
            known.clear();
        }
        known.insert(code.to_owned(), None);
        net.post(Command::ResolveInvite { code: code.to_owned() });
        None
    }

    pub fn store(&self, code: String, invite: Option<Invite>) {
        self.known.borrow_mut().insert(code, Some(invite));
    }
}

/// "Invite People": the link made for `channel`, once Discord answers.
pub(super) struct InviteModal {
    pub channel: Snowflake,
    pub link: Option<Result<SharedString, SharedString>>,
    pub copied: bool,
}

impl Emerald {
    /// Asks Discord for an invite to `channel` and shows the dialog.
    /// `location` is the web client's name for where it was asked.
    pub(super) fn open_invite(&mut self, channel: Snowflake, location: &'static str, cx: &mut Context<Self>) {
        self.place_menu = None;
        self.net.post(Command::CreateInvite { channel, location });
        self.overlay = Some(Overlay::Invite(InviteModal {
            channel,
            link: None,
            copied: false,
        }));
        cx.notify();
    }

    pub(super) fn on_invite_created(&mut self, channel: Snowflake, invite: Result<Invite, String>) {
        let Some(Overlay::Invite(modal)) = &mut self.overlay else { return };
        if modal.channel == channel {
            modal.link = Some(match invite {
                Ok(invite) => Ok(format!("https://discord.gg/{}", invite.code).into()),
                Err(error) => Err(error.into()),
            });
        }
    }

    pub(super) fn render_invite_modal(&self, modal: &InviteModal, cx: &mut Context<Self>) -> AnyElement {
        let guild = self.spaces.guild_id_of(modal.channel).and_then(|id| self.spaces.iter().find(|s| s.id == id));
        let title = t!("invite.title", name = guild.map(|g| g.name.to_string()).unwrap_or_default());
        let channel = self.spaces.channel(modal.channel).and_then(|c| c.name.clone()).unwrap_or_default();
        let (link, error) = match &modal.link {
            None => (SharedString::from(t!("common.loading")), None),
            Some(Ok(link)) => (link.clone(), None),
            Some(Err(error)) => (SharedString::default(), Some(error.clone())),
        };
        let ready = matches!(modal.link, Some(Ok(_)));
        let copy_label = if modal.copied { t!("invite.copied") } else { t!("invite.copy") };
        div()
            .id("invite-scrim")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme::scrim())
            .occlude()
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| this.close_overlay(window, cx)))
            .child(
                div()
                    .id("invite-modal")
                    .w(px(440.))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_4()
                    .rounded_lg()
                    .border_1()
                    .border_color(theme::border())
                    .bg(rgb(theme::SURFACE_HIGH))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .text_size(px(20.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT_STRONG))
                            .child(title),
                    )
                    .child(div().text_size(px(14.)).text_color(theme::muted()).child(format!("# {channel}")))
                    .child(widgets::label(t!("invite.link_label")))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .p_1()
                            .pl_3()
                            .rounded_md()
                            .bg(rgb(theme::BASE_LOWEST))
                            .child(div().flex_1().min_w(px(0.)).truncate().text_color(rgb(theme::TEXT_DEFAULT)).child(link.clone()))
                            .child(
                                widgets::button("invite-copy", copy_label, if modal.copied { ButtonKind::Positive } else { ButtonKind::Brand })
                                    .h(px(32.))
                                    .when(!ready, |b| b.opacity(0.5))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if !ready {
                                            return;
                                        }
                                        cx.write_to_clipboard(ClipboardItem::new_string(link.to_string()));
                                        if let Some(Overlay::Invite(modal)) = &mut this.overlay {
                                            modal.copied = true;
                                        }
                                        cx.notify();
                                    })),
                            ),
                    )
                    .children(error.map(|error| div().text_size(px(14.)).text_color(rgb(theme::FEEDBACK_CRITICAL)).child(error)))
                    .child(div().text_xs().text_color(theme::muted()).child(t!("invite.expires"))),
            )
            .into_any_element()
    }

    /// Cards for the invites linked in `content`.
    pub(super) fn render_invite_cards(&self, content: &str, message: Snowflake, ix: usize, cx: &mut Context<Self>) -> Vec<AnyElement> {
        if !content.contains("http") {
            return Vec::new();
        }
        markdown::invite_codes(content, CARDS_PER_MESSAGE)
            .into_iter()
            .enumerate()
            .map(|(n, code)| self.render_invite_card(code, message, ix * CARDS_PER_MESSAGE + n, cx))
            .collect()
    }

    fn render_invite_card(&self, code: &str, message: Snowflake, id: usize, cx: &mut Context<Self>) -> AnyElement {
        let found = self.invite_cards.get(code, &*self.net);
        let card = div()
            .w(px(432.))
            .max_w_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .rounded_lg()
            .bg(rgb(theme::BASE_LOWEST))
            .border_1()
            .border_color(theme::border())
            .child(widgets::label(t!("invite.card_title")));
        let guild = match &found {
            None => return card.child(div().text_color(theme::muted()).child(t!("common.loading"))).into_any_element(),
            Some(invite) => invite.as_ref().and_then(|i| Some((i, i.guild.as_ref()?))),
        };
        let Some((invite, guild)) = guild else {
            return card
                .child(div().font_weight(FontWeight::SEMIBOLD).text_color(rgb(theme::FEEDBACK_CRITICAL)).child(t!("invite.invalid")))
                .into_any_element();
        };
        let joined = self.spaces.index(guild.id).is_some();
        let name: SharedString = guild.name.clone().into();
        let icon = guild.icon_url().and_then(|url| self.avatar_image(&url));
        let counts = match (invite.approximate_presence_count, invite.approximate_member_count) {
            (Some(online), Some(members)) => Some(t!("invite.counts", online = online, members = members)),
            _ => None,
        };
        let (guild_id, code) = (guild.id, code.to_owned());
        card.child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .size(px(50.))
                        .flex_none()
                        .rounded_xl()
                        .overflow_hidden()
                        .bg(theme::hover())
                        .flex()
                        .items_center()
                        .justify_center()
                        .map(|tile| match icon {
                            Some(image) => tile.child(img(image).size_full()),
                            None => tile.text_color(rgb(theme::TEXT_STRONG)).child(name.chars().next().unwrap_or('?').to_string()),
                        }),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .flex()
                        .flex_col()
                        .child(div().truncate().font_weight(FontWeight::SEMIBOLD).text_color(rgb(theme::TEXT_STRONG)).child(name))
                        .children(counts.map(|counts| div().text_xs().text_color(theme::muted()).child(counts))),
                )
                .child(
                    widgets::button(("invite-join", id), if joined { t!("invite.joined") } else { t!("invite.join") }, ButtonKind::Positive)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if let Some(ix) = this.spaces.index(guild_id) {
                                this.select_space(ix, window, cx);
                            } else if this.run(Action::JoinGuild, Command::JoinGuild { code: code.clone(), message: Some(message) }, cx) {
                                this.select_new_guild = true;
                            }
                        })),
                ),
        )
        .into_any_element()
    }
}
