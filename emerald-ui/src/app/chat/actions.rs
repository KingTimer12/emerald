//! What can be done to a message: reply, edit, pin, copy and delete. The
//! toolbar shown on hover and the context menu offer the same actions.

pub(super) mod bulk;
mod editing;
mod menu;

use gpui::{
    AnyElement, ClipboardItem, Context, Div, Focusable, FontWeight, MouseButton, SharedString, Window, div,
    prelude::*, rgb,
};

use super::super::super::icons::{self, icon};
use super::super::super::theme;
use super::super::state::{Msg, Reply};
use super::super::{Emerald, Overlay, shown, widgets};
use super::reactions::{QUICK_REACTIONS, emoji_label};
use emerald_discord::model::{MessageUpdate, Snowflake, message_kind};
use emerald_discord::{Command, ReactionSource};
use emerald_discord::permissions;

/// The message being edited in place.
pub(in super::super) struct Editing {
    pub channel: Snowflake,
    pub message: Snowflake,
}

/// The message a send will answer, shown above the message box.
pub(in super::super) struct Replying {
    pub channel: Snowflake,
    pub message: Snowflake,
    pub preview: Reply,
    pub name: SharedString,
    /// Pings the author; off when replying to oneself.
    pub mention: bool,
}

/// The context menu of a message, open at `position`.
pub(in super::super) struct MessageMenu {
    pub channel: Snowflake,
    pub message: Snowflake,
    pub anchor: super::super::widgets::popover::Anchor,
}

/// What the current user may do to one message.
#[derive(Clone, Copy)]
struct Rights {
    reply: bool,
    edit: bool,
    pin: bool,
    delete: bool,
}

impl Emerald {
    fn rights(&self, channel: Snowflake, msg: &Msg) -> Rights {
        let space = self.spaces.current();
        let mine = self.people.is_me(msg.author_id);
        // DMs report every permission; there only pinning is open to both sides.
        let granted = |bits: u64| !space.is_dms() && space.permissions(channel) & bits != 0;
        let regular = matches!(msg.kind, message_kind::DEFAULT | message_kind::REPLY);
        let sent = !msg.pending;
        Rights {
            reply: sent && regular && self.can_send(channel),
            edit: mine && msg.editable_text().is_some(),
            pin: sent
                && regular
                && (space.is_dms() || granted(permissions::MANAGE_MESSAGES | permissions::PIN_MESSAGES)),
            delete: sent && (mine || granted(permissions::MANAGE_MESSAGES)),
        }
    }

    /// A message went away: nothing may keep pointing at it.
    pub(in super::super) fn message_gone(&mut self, channel: Snowflake, message: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        self.forget_bulk(channel, message);
        let target = |c: Snowflake, m: Snowflake| c == channel && m == message;
        if self.edit.as_ref().is_some_and(|e| target(e.channel, e.message)) {
            self.cancel_edit(window, cx);
        }
        if self.composer.reply.as_ref().is_some_and(|r| target(r.channel, r.message)) {
            self.composer.reply = None;
        }
        if self.message_menu.as_ref().is_some_and(|m| target(m.channel, m.message)) {
            self.message_menu = None;
        }
        if matches!(self.overlay, Some(Overlay::DeleteMessage { channel: c, message: m }) if target(c, m)) {
            self.close_overlay(window, cx);
        }
    }

    /// Escape closes the innermost thing open: the menu, an overlay, the
    /// edit box, then the reply.
    pub(in super::super) fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_reaction_picker(window, cx) || self.dismiss_suggestions(cx) {
        } else if self.message_menu.take().is_some()
            || self.place_menu.take().is_some()
            || self.user_menu.take().is_some()
            || self.audio_menu.take().is_some()
            || self.inbox.take().is_some()
            || self.own_popover.take().is_some()
            || std::mem::take(&mut self.plus_menu)
        {
            cx.notify();
        } else if self.overlay.is_some() {
            self.close_overlay(window, cx);
        } else if self.edit.is_some() {
            self.cancel_edit(window, cx);
        } else if self.composer.reply.take().is_some() {
            cx.notify();
        }
    }

    fn redraw_message(&self, channel: Snowflake, id: Snowflake) {
        let view = shown(&self.messages, self.selected_channel, channel);
        self.history.redraw(channel, id, view);
    }

    pub(super) fn is_editing(&self, msg: &Msg) -> bool {
        self.edit
            .as_ref()
            .is_some_and(|e| Some(e.channel) == self.selected_channel && e.message == msg.id)
    }
}
