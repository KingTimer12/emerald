//! Small dialogs over the window: a question before something that cannot
//! be undone at a click (leaving a server or group, blocking), and a one
//! line text field (a group's name, a friend's nickname).

use gpui::{AnyElement, Context, Focusable, FontWeight, MouseButton, SharedString, Window, div, prelude::*, rgb};

use super::super::theme;
use super::widgets::{self, ButtonKind};
use super::{Emerald, Overlay};
use emerald_discord::Command;
use emerald_discord::model::Snowflake;
use crate::zoom::zoomed;

/// What a confirmation dialog asks about.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Ask {
    LeaveGuild(Snowflake),
    LeaveGroup(Snowflake),
    Block(Snowflake),
    RemoveRecipient { channel: Snowflake, user: Snowflake },
    /// Kick, and ban, a member of `guild`.
    Kick { guild: Snowflake, user: Snowflake },
    Ban { guild: Snowflake, user: Snowflake },
    /// Deleting the messages picked in `channel`; the picks wait in `Emerald::bulk`.
    BulkDelete { channel: Snowflake, count: usize },
}

/// What a text dialog sets.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Prompt {
    RenameGroup(Snowflake),
    Nickname(Snowflake),
}

impl Emerald {
    pub(super) fn ask(&mut self, ask: Ask, cx: &mut Context<Self>) {
        self.place_menu = None;
        self.user_menu = None;
        self.overlay = Some(Overlay::Confirm(ask));
        cx.notify();
    }

    /// Opens a text dialog holding the current value.
    pub(super) fn prompt(&mut self, prompt: Prompt, window: &mut Window, cx: &mut Context<Self>) {
        self.place_menu = None;
        self.user_menu = None;
        let current = match prompt {
            Prompt::RenameGroup(channel) => self.spaces.channel(channel).and_then(|c| c.name.clone()),
            Prompt::Nickname(user) => self.people.nickname(user).map(str::to_owned),
        };
        self.prompt_input.update(cx, |input, cx| input.set_text(current.unwrap_or_default(), cx));
        window.focus(&self.prompt_input.read(cx).focus_handle(cx));
        self.overlay = Some(Overlay::Prompt(prompt));
        cx.notify();
    }

    fn confirm(&mut self, ask: Ask, window: &mut Window, cx: &mut Context<Self>) {
        self.close_overlay(window, cx);
        let command = match ask {
            Ask::LeaveGuild(guild) => Command::LeaveGuild { guild },
            Ask::LeaveGroup(channel) => Command::CloseChannel { channel },
            Ask::Block(user) => Command::Block { user, block: true },
            Ask::RemoveRecipient { channel, user } => Command::RemoveRecipient { channel, user },
            Ask::Kick { guild, user } => Command::Kick { guild, user },
            Ask::Ban { guild, user } => Command::Ban { guild, user, delete_seconds: 0 },
            Ask::BulkDelete { channel, .. } => match self.take_bulk(channel) {
                Some(messages) => Command::BulkDelete { channel, messages },
                None => return,
            },
        };
        self.net.post(command);
    }

    /// Enter in the text dialog, or its Save button. An empty value clears
    /// the name, as on Discord.
    pub(super) fn submit_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Overlay::Prompt(prompt)) = self.overlay else { return };
        let text = self.prompt_input.read(cx).text().trim().to_owned();
        let value = (!text.is_empty()).then_some(text);
        self.close_overlay(window, cx);
        self.net.post(match prompt {
            Prompt::RenameGroup(channel) => Command::RenameGroup { channel, name: value },
            Prompt::Nickname(user) => Command::SetNickname { user, nickname: value },
        });
    }

    pub(super) fn render_confirm(&self, ask: Ask, cx: &mut Context<Self>) -> AnyElement {
        let guild_name = |id| self.spaces.iter().find(|s| s.id == id).map(|s| s.name.to_string()).unwrap_or_default();
        let user_name = |id| self.people.friend_name(id).unwrap_or("?").to_owned();
        let member_name = |id| self.people.display_name(id).unwrap_or("?").to_owned();
        let group_name = |id: Snowflake| self.spaces.dms().label(id).map(|l| l.to_string()).unwrap_or_default();
        let (title, body, button): (String, String, &'static str) = match ask {
            Ask::LeaveGuild(guild) => (
                t!("leave.server_title", name = guild_name(guild)),
                t!("leave.server_body", name = guild_name(guild)),
                t!("leave.server"),
            ),
            Ask::LeaveGroup(channel) => (
                t!("leave.group_title", name = group_name(channel)),
                t!("leave.group_body", name = group_name(channel)),
                t!("leave.group"),
            ),
            Ask::Block(user) => (
                t!("block.title", name = user_name(user)),
                t!("block.body", name = user_name(user)),
                t!("block.block"),
            ),
            Ask::RemoveRecipient { user, .. } => (
                t!("group.remove_title", name = user_name(user)),
                t!("group.remove_body", name = user_name(user)),
                t!("group.remove"),
            ),
            Ask::Kick { user, .. } => (
                t!("moderation.kick_title", name = member_name(user)),
                t!("moderation.kick_body", name = member_name(user)),
                t!("moderation.kick"),
            ),
            Ask::Ban { user, .. } => (
                t!("moderation.ban_title", name = member_name(user)),
                t!("moderation.ban_body", name = member_name(user)),
                t!("moderation.ban"),
            ),
            Ask::BulkDelete { count, .. } => (
                t!("bulk.delete_title", count = count),
                t!("bulk.delete_body").to_owned(),
                t!("bulk.delete"),
            ),
        };
        let content = div().text_color(rgb(theme::text_default())).child(body).into_any_element();
        let confirm = widgets::button("dialog-confirm", button, ButtonKind::Danger)
            .on_click(cx.listener(move |this, _, window, cx| this.confirm(ask, window, cx)));
        dialog(title.into(), content, confirm, cx)
    }

    pub(super) fn render_prompt(&self, prompt: Prompt, cx: &mut Context<Self>) -> AnyElement {
        let (title, label) = match prompt {
            Prompt::RenameGroup(_) => (t!("group.rename"), t!("group.name")),
            Prompt::Nickname(_) => (t!("friends.nickname_title"), t!("friends.nickname")),
        };
        let content = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(widgets::label(label))
            .child(self.prompt_input.clone())
            .when(matches!(prompt, Prompt::Nickname(_)), |d| {
                d.child(div().text_xs().text_color(theme::muted()).child(t!("friends.nickname_hint")))
            })
            .into_any_element();
        let save = widgets::button("dialog-save", t!("common.save"), ButtonKind::Brand)
            .on_click(cx.listener(|this, _, window, cx| this.submit_prompt(window, cx)));
        dialog(title.into(), content, save, cx)
    }
}

/// A centered card over a scrim with a title, `content`, and Cancel beside
/// `action` at the bottom; clicking outside cancels.
pub(super) fn dialog(title: SharedString, content: AnyElement, action: gpui::Stateful<gpui::Div>, cx: &mut Context<Emerald>) -> AnyElement {
    div()
        .id("dialog-scrim")
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
                .id("dialog")
                .w(zoomed(440.))
                .rounded(theme::radius::modal())
                .overflow_hidden()
                .border_1()
                .border_color(theme::border())
                .bg(rgb(theme::surface_high()))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .p_4()
                        .child(
                            div()
                                .text_size(zoomed(20.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(theme::text_strong()))
                                .child(title),
                        )
                        .child(content),
                )
                .child(
                    div()
                        .p_4()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .bg(rgb(theme::base_low()))
                        .child(
                            widgets::button("dialog-cancel", t!("common.cancel"), ButtonKind::Secondary)
                                .on_click(cx.listener(|this, _, window, cx| this.close_overlay(window, cx))),
                        )
                        .child(action),
                ),
        )
        .into_any_element()
}
