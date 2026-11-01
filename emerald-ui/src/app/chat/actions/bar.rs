//! The hover bar in the top right corner of a message, in Discord 2026's
//! form: quick reactions, then the actions, and more of them while Shift
//! is held. Which buttons show lives in
//! [`state::hover_bar`](super::super::super::state::hover_bar).

use super::*;
use super::super::super::state::hover_bar::{self, BarButton, BarRights};
use super::super::super::widgets::popover::{Anchor, Tracked};
use crate::zoom::zoomed;
use emerald_discord::SaveSource;

/// Width and height of a bar button.
const PITCH: f32 = 28.;

impl Emerald {
    pub(in super::super) fn message_toolbar(&self, msg: &Msg, ix: usize, group: SharedString, cx: &mut Context<Self>) -> Option<AnyElement> {
        let channel = self.selected_channel?;
        if msg.pending || self.is_editing(msg) {
            return None;
        }
        let id = msg.id;
        let open = self.message_menu.as_ref().is_some_and(|m| m.message == id)
            || self.reaction_picker.as_ref().is_some_and(|p| p.message == Some(id));
        let rights = self.bar_rights(channel, msg);
        let quick: Vec<_> = if rights.react {
            self.recent_emojis().take(QUICK_REACTIONS).collect()
        } else {
            Vec::new()
        };
        let expanded = hover_bar::expanded(self.shift_held, self.window_active, open);
        let own = self.people.is_me(msg.author_id);
        let bookmarked = self.saved.is_bookmarked(id);
        let button = |name: &'static str, glyph: &'static str, tip: SharedString, danger: bool| {
            div()
                .id((name, ix))
                .size(zoomed(PITCH))
                .flex()
                .items_center()
                .justify_center()
                .rounded(theme::radius::control())
                .cursor_pointer()
                .hover(|s| s.bg(theme::hover()))
                .tooltip(widgets::tooltip(tip))
                .child(icon(glyph, 20., if danger { theme::danger() } else { theme::interactive() }))
        };
        let children = hover_bar::buttons(own, expanded, rights, quick.len()).into_iter().map(|kind| match kind {
            BarButton::Quick(n) => {
                let emoji = quick[n].clone();
                div()
                    .id(("msg-quick", ix * QUICK_REACTIONS + n))
                    .size(zoomed(PITCH))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(theme::radius::control())
                    .cursor_pointer()
                    .hover(|s| s.bg(theme::hover()))
                    .tooltip(widgets::tooltip(emoji_label(&emoji)))
                    .child(self.emoji_image(&emoji, 20.))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_reaction(channel, id, emoji.clone(), ReactionSource::HoverBar, cx)
                    }))
                    .into_any_element()
            }
            BarButton::Divider => div().mx_1().w(zoomed(1.)).h(zoomed(20.)).bg(theme::border()).into_any_element(),
            BarButton::AddReaction => {
                let tracked = Tracked::default();
                tracked
                    .track(button("msg-react", icons::REACTION, t!("reaction.add").into(), false))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_reaction_picker(channel, id, Anchor::left_of(tracked.bounds()), window, cx)
                    }))
                    .into_any_element()
            }
            BarButton::Reply => button("msg-reply", icons::REPLY, t!("message.reply").into(), false)
                .on_click(cx.listener(move |this, _, window, cx| this.start_reply(channel, id, window, cx)))
                .into_any_element(),
            BarButton::Edit => button("msg-edit", icons::EDIT, t!("message.edit").into(), false)
                .on_click(cx.listener(move |this, _, window, cx| this.start_edit(channel, id, window, cx)))
                .into_any_element(),
            BarButton::Forward => button("msg-forward", icons::FORWARD, t!("forward.action").into(), false)
                .on_click(cx.listener(move |this, _, window, cx| this.open_forward(channel, id, window, cx)))
                .into_any_element(),
            BarButton::Thread => button("msg-thread", icons::THREAD, t!("thread.create").into(), false)
                .on_click(cx.listener(move |this, _, window, cx| this.open_create_thread(channel, Some(id), window, cx)))
                .into_any_element(),
            BarButton::CopyId => button("msg-copy-id", icons::COPY, t!("message.copy_id").into(), false)
                .on_click(cx.listener(move |this, _, _, cx| this.copy_message(channel, id, true, cx)))
                .into_any_element(),
            BarButton::CopyLink => button("msg-copy-link", icons::LINK, t!("message.copy_link").into(), false)
                .on_click(cx.listener(move |this, _, _, cx| this.copy_message_link(channel, id, cx)))
                .into_any_element(),
            BarButton::MarkUnread => button("msg-unread", icons::MARK_UNREAD, t!("message.mark_unread").into(), false)
                .on_click(cx.listener(move |this, _, _, cx| this.mark_unread_from(channel, id, cx)))
                .into_any_element(),
            BarButton::Bookmark => {
                let (glyph, tip) = if bookmarked {
                    (icons::BOOKMARK_FILLED, t!("saved.remove_bookmark"))
                } else {
                    (icons::BOOKMARK, t!("saved.bookmark"))
                };
                button("msg-bookmark", glyph, tip.into(), false)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_bookmark(channel, id, SaveSource::MessageToolbar);
                        cx.notify();
                    }))
                    .into_any_element()
            }
            BarButton::Pin => {
                let tip = if msg.pinned { t!("message.unpin") } else { t!("message.pin") };
                button("msg-pin", icons::PIN, tip.into(), false)
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_pin(channel, id, cx)))
                    .into_any_element()
            }
            BarButton::Delete => button("msg-delete", icons::TRASH, t!("message.delete").into(), true)
                .on_click(cx.listener(move |this, event: &gpui::ClickEvent, _, cx| {
                    this.ask_delete(channel, id, event.modifiers().shift, cx)
                }))
                .into_any_element(),
            BarButton::More => {
                let tracked = Tracked::default();
                tracked
                    .track(button("msg-more", icons::MORE, t!("message.more").into(), false))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_message_menu(channel, id, Anchor::left_of(tracked.bounds()), cx)
                    }))
                    .into_any_element()
            }
        });
        Some(
            div()
                .absolute()
                .top(zoomed(-16.))
                .right(zoomed(14.))
                .flex()
                .items_center()
                .p(zoomed(2.))
                .rounded(theme::radius::control())
                .border_1()
                .border_color(theme::border_normal())
                .bg(rgb(theme::surface_high()))
                .shadow_md()
                .when(!open, |bar| bar.invisible().group_hover(group, |s| s.visible()))
                .children(children)
                .into_any_element(),
        )
    }

    fn bar_rights(&self, channel: Snowflake, msg: &Msg) -> BarRights {
        let rights = self.rights(channel, msg);
        let regular = matches!(msg.kind, message_kind::DEFAULT | message_kind::REPLY);
        BarRights {
            react: self.may_react(channel, msg, true),
            reply: rights.reply,
            edit: rights.edit,
            forward: regular && !msg.pending,
            thread: self.may_start_thread(channel, msg),
            pin: rights.pin,
            delete: rights.delete,
            bookmark: self.saved.reminder(msg.id).is_none(),
            mark_unread: self.may_mark_unread(channel),
        }
    }

    /// Shift pressed or released: the bar under the pointer changes form.
    pub(in super::super::super) fn on_modifiers(&mut self, shift: bool, cx: &mut Context<Self>) {
        if self.shift_held != shift {
            self.shift_held = shift;
            cx.notify();
        }
    }
}
