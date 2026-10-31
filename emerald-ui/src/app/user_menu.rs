//! The right-click menu of a person, wherever they are drawn: their
//! profile, a message, a note, the friend nickname, removing them from a
//! group, ignoring and blocking.

use gpui::{AnyElement, Context, MouseButton, MouseDownEvent, div, prelude::*, rgb};

use super::super::theme;
use super::dialogs::{Ask, Prompt};
use super::place_menu::item;
use super::widgets::popover::Anchor;
use super::Emerald;
use emerald_discord::Command;
use emerald_discord::model::{ChannelKind, Snowflake};
use crate::zoom::zoomed;

pub(super) struct UserMenu {
    user: Snowflake,
    /// The server whose member was clicked, for their profile there.
    guild: Option<Snowflake>,
    /// The group DM listing them, when the click came from it.
    group: Option<Snowflake>,
    anchor: Anchor,
}

impl Emerald {
    pub(super) fn open_user_menu(
        &mut self,
        user: Snowflake,
        guild: Option<Snowflake>,
        group: Option<Snowflake>,
        anchor: Anchor,
        cx: &mut Context<Self>,
    ) {
        self.place_menu = None;
        self.message_menu = None;
        self.user_menu = Some(UserMenu { user, guild, group, anchor });
        cx.notify();
    }

    /// A right-click handler opening the menu of `user`.
    pub(super) fn on_user_right_click(
        &self,
        user: Snowflake,
        guild: Option<Snowflake>,
        group: Option<Snowflake>,
        cx: &mut Context<Self>,
    ) -> impl Fn(&MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static {
        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
            cx.stop_propagation();
            this.open_user_menu(user, guild, group, Anchor::pointer(event.position), cx)
        })
    }

    pub(super) fn render_user_menu(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let menu = self.user_menu.as_ref()?;
        let (user, guild, group) = (menu.user, menu.guild, menu.group);
        let me = self.people.is_me(user);
        let mut items: Vec<AnyElement> = Vec::new();
        let separator = || div().my_1().h(zoomed(1.)).bg(theme::border()).into_any_element();
        items.push(
            item(("user-profile", 0), t!("user_menu.profile").into(), None)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.user_menu = None;
                    this.open_profile(user, guild, window, cx)
                }))
                .into_any_element(),
        );
        if !me {
            items.push(
                item(("user-message", 0), t!("user_menu.message").into(), None)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.user_menu = None;
                        this.open_dm(user, window, cx)
                    }))
                    .into_any_element(),
            );
            items.push(
                item(("user-note", 0), t!("user_menu.note").into(), None)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.user_menu = None;
                        this.open_profile(user, guild, window, cx)
                    }))
                    .into_any_element(),
            );
        }
        if self.people.is_friend(user) {
            let label = if self.people.nickname(user).is_some() { t!("user_menu.edit_nickname") } else { t!("user_menu.add_nickname") };
            items.push(
                item(("user-nickname", 0), label.into(), None)
                    .on_click(cx.listener(move |this, _, window, cx| this.prompt(Prompt::Nickname(user), window, cx)))
                    .into_any_element(),
            );
        }
        // Only the owner of a group removes people from it.
        let owned_group = group.filter(|channel| {
            self.spaces.channel(*channel).is_some_and(|c| {
                c.kind == ChannelKind::GROUP_DM && c.owner_id == Some(self.people.my_id()) && c.recipient_ids.contains(&user)
            })
        });
        if let Some(channel) = owned_group.filter(|_| !me) {
            items.push(separator());
            items.push(
                item(("user-remove", 0), t!("group.remove").into(), None)
                    .text_color(rgb(theme::feedback_critical()))
                    .on_click(cx.listener(move |this, _, _, cx| this.ask(Ask::RemoveRecipient { channel, user }, cx)))
                    .into_any_element(),
            );
        }
        // Moderation acts on the server the member was clicked in, and never on oneself.
        let moderation = match guild.filter(|guild| !me && self.spaces.current().id == *guild) {
            Some(guild) => self.moderation_items(guild, user, cx),
            None => Vec::new(),
        };
        if !moderation.is_empty() {
            items.push(separator());
            items.extend(moderation);
        }
        if !me {
            items.push(separator());
            if self.people.is_friend(user) {
                items.push(
                    item(("user-remove-friend", 0), t!("friends.remove_friend").into(), None)
                        .text_color(rgb(theme::feedback_critical()))
                        .on_click(cx.listener(move |this, _, _, cx| this.ask(Ask::RemoveFriend(user), cx)))
                        .into_any_element(),
                );
            }
            let blocked = self.people.is_blocked(user);
            if !blocked {
                let ignored = self.people.is_ignored(user);
                let label = if ignored { t!("user_menu.unignore") } else { t!("user_menu.ignore") };
                items.push(
                    item(("user-ignore", 0), label.into(), None)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.user_menu = None;
                            this.net.post(Command::Ignore { user, ignore: !ignored });
                            cx.notify();
                        }))
                        .into_any_element(),
                );
            }
            let block = item(
                ("user-block", 0),
                if blocked { t!("user_menu.unblock") } else { t!("user_menu.block") }.into(),
                None,
            )
            .text_color(rgb(theme::feedback_critical()));
            items.push(
                block
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if blocked {
                            this.user_menu = None;
                            this.net.post(Command::Block { user, block: false });
                            cx.notify();
                        } else {
                            this.ask(Ask::Block(user), cx);
                        }
                    }))
                    .into_any_element(),
            );
        }
        let list = super::widgets::popover::panel("user-menu", 220.)
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.user_menu = None;
                cx.notify();
            }))
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .children(items);
        Some(super::widgets::popover::at(menu.anchor, list))
    }
}
