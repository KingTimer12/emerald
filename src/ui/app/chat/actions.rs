//! What can be done to a message: reply, edit, pin, copy and delete. The
//! toolbar shown on hover and the context menu offer the same actions.

use gpui::{
    AnyElement, ClipboardItem, Context, Div, Focusable, FontWeight, MouseButton, Pixels, Point, SharedString, Window,
    anchored, deferred, div, prelude::*, px, rgb,
};

use super::super::super::icons::{self, icon};
use super::super::super::theme;
use super::super::state::{Msg, Reply};
use super::super::{Emerald, Overlay, shown, widgets};
use super::reactions::{QUICK_REACTIONS, emoji_label};
use crate::discord::model::{MessageUpdate, Snowflake, message_kind};
use crate::discord::{Command, ReactionSource};
use crate::discord::permissions;

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
    pub position: Point<Pixels>,
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

    pub(in super::super) fn start_reply(&mut self, channel: Snowflake, id: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        let Some((_, msg)) = self.history.find(channel, id) else { return };
        let mine = self.people.is_me(msg.author_id);
        let (name, _) = self.member_style(msg.author_id, &msg.author);
        self.composer.reply = Some(Replying {
            channel,
            message: id,
            preview: Reply::of(msg),
            name,
            mention: !mine,
        });
        self.message_menu = None;
        window.focus(&self.composer.input.focus_handle(cx));
        cx.notify();
    }

    pub(in super::super) fn start_edit(&mut self, channel: Snowflake, id: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = self.history.find(channel, id).and_then(|(_, msg)| msg.editable_text()) else { return };
        let text = text.to_owned();
        self.stop_editing(cx);
        self.edit = Some(Editing { channel, message: id });
        self.message_menu = None;
        self.edit_input.update(cx, |input, cx| input.set_text(text, cx));
        self.redraw_message(channel, id);
        window.focus(&self.edit_input.focus_handle(cx));
        cx.notify();
    }

    /// Up in an empty message box edits the user's last message, as on Discord.
    pub(in super::super) fn edit_last(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(channel) = self.selected_channel.filter(|_| !self.friends_open) else { return };
        let composer = self.composer.input.read(cx);
        if !composer.focus_handle(cx).is_focused(window) || !composer.text().is_empty() {
            return;
        }
        if let Some(id) = self.history.last_editable(channel, self.people.my_id()) {
            self.start_edit(channel, id, window, cx);
        }
    }

    /// Closes the edit box without saving, if one is open.
    fn stop_editing(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(edit) = self.edit.take() else { return false };
        self.redraw_message(edit.channel, edit.message);
        cx.notify();
        true
    }

    fn cancel_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.stop_editing(cx) {
            window.focus(&self.composer.input.focus_handle(cx));
        }
    }

    /// Enter in the edit box. The new text shows at once; the gateway echo
    /// confirms it.
    pub(in super::super) fn save_edit(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(edit) = self.edit.as_ref() else { return };
        let (channel, message) = (edit.channel, edit.message);
        let unchanged = self
            .history
            .find(channel, message)
            .and_then(|(_, msg)| msg.editable_text())
            .is_some_and(|old| old.trim() == text.trim());
        self.cancel_edit(window, cx);
        if unchanged {
            return;
        }
        let update = MessageUpdate {
            id: message,
            channel_id: channel,
            content: Some(text.clone()),
            edited_timestamp: Some(String::new()),
            pinned: None,
            embeds: None,
            mentions: None,
            mention_roles: None,
            mention_everyone: None,
            thread: None,
        };
        let view = shown(&self.messages, self.selected_channel, channel);
        self.history.edit(update, view);
        self.net.post(Command::EditMessage {
            channel,
            message,
            content: text,
        });
    }

    fn open_message_menu(&mut self, channel: Snowflake, message: Snowflake, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.message_menu = Some(MessageMenu {
            channel,
            message,
            position,
        });
        cx.notify();
    }

    /// Deletes at once with shift held, else asks first, like Discord.
    fn ask_delete(&mut self, channel: Snowflake, message: Snowflake, shift: bool, cx: &mut Context<Self>) {
        self.message_menu = None;
        if shift {
            self.delete_message(channel, message, cx);
        } else {
            self.overlay = Some(Overlay::DeleteMessage { channel, message });
            cx.notify();
        }
    }

    fn delete_message(&mut self, channel: Snowflake, message: Snowflake, cx: &mut Context<Self>) {
        self.net.post(Command::DeleteMessage { channel, message });
        cx.notify();
    }

    fn toggle_pin(&mut self, channel: Snowflake, message: Snowflake, cx: &mut Context<Self>) {
        self.message_menu = None;
        let Some((_, msg)) = self.history.find(channel, message) else { return };
        let pin = !msg.pinned;
        self.net.post(Command::PinMessage { channel, message, pin });
        cx.notify();
    }

    fn copy_message(&mut self, channel: Snowflake, message: Snowflake, id: bool, cx: &mut Context<Self>) {
        self.message_menu = None;
        let text = if id {
            Some(message.to_string())
        } else {
            self.history.find(channel, message).map(|(_, msg)| msg.content.to_string())
        };
        if let Some(text) = text {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
        cx.notify();
    }

    /// A message went away: nothing may keep pointing at it.
    pub(in super::super) fn message_gone(&mut self, channel: Snowflake, message: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
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
        } else if self.message_menu.take().is_some() {
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

    /// The hover toolbar in the top right corner of a message.
    pub(super) fn message_toolbar(&self, msg: &Msg, ix: usize, group: SharedString, cx: &mut Context<Self>) -> Option<AnyElement> {
        let channel = self.selected_channel?;
        if msg.pending || self.is_editing(msg) {
            return None;
        }
        let rights = self.rights(channel, msg);
        let id = msg.id;
        let open = self.message_menu.as_ref().is_some_and(|m| m.message == id)
            || self.reaction_picker.as_ref().is_some_and(|p| p.message == Some(id));
        let react = self.may_react(channel, msg, true);
        let quick: Vec<_> = if react {
            self.recent_emojis().take(QUICK_REACTIONS).collect()
        } else {
            Vec::new()
        };
        let button = |name: &'static str, glyph: &'static str, tip: &'static str| {
            div()
                .id((name, ix))
                .size(px(32.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_md()
                .cursor_pointer()
                .hover(|s| s.bg(theme::hover()))
                .tooltip(widgets::tooltip(tip))
                .child(icon(glyph, 20., theme::INTERACTIVE))
        };
        Some(
            div()
                .absolute()
                .top(px(-16.))
                .right(px(14.))
                .flex()
                .p(px(2.))
                .rounded_lg()
                .border_1()
                .border_color(theme::border())
                .bg(rgb(theme::BASE_LOWER))
                .when(!open, |bar| bar.invisible().group_hover(group, |s| s.visible()))
                .children(quick.into_iter().enumerate().map(|(n, emoji)| {
                    let label = emoji_label(&emoji);
                    div()
                        .id(("msg-quick", ix * QUICK_REACTIONS + n))
                        .size(px(32.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_md()
                        .cursor_pointer()
                        .hover(|s| s.bg(theme::hover()))
                        .tooltip(widgets::tooltip(label))
                        .child(self.emoji_image(&emoji, 20.))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.toggle_reaction(channel, id, emoji.clone(), ReactionSource::HoverBar, cx)
                        }))
                }))
                .when(react, |bar| {
                    bar.child(button("msg-react", icons::REACTION, t!("reaction.add")).on_click(cx.listener(
                        move |this, event: &gpui::ClickEvent, window, cx| {
                            this.open_reaction_picker(channel, id, event.position(), window, cx)
                        },
                    )))
                })
                .when(rights.reply, |bar| {
                    bar.child(button("msg-reply", icons::REPLY, t!("message.reply")).on_click(cx.listener(
                        move |this, _, window, cx| this.start_reply(channel, id, window, cx),
                    )))
                })
                .when(rights.edit, |bar| {
                    bar.child(button("msg-edit", icons::EDIT, t!("message.edit")).on_click(cx.listener(
                        move |this, _, window, cx| this.start_edit(channel, id, window, cx),
                    )))
                })
                .child(button("msg-more", icons::MORE, t!("message.more")).on_click(cx.listener(
                    move |this, event: &gpui::ClickEvent, _, cx| {
                        this.open_message_menu(channel, id, event.position(), cx)
                    },
                )))
                .into_any_element(),
        )
    }

    /// Right click on a message opens its menu where the pointer is.
    pub(super) fn on_message_right_click(&self, ix: usize, cx: &mut Context<Self>) -> impl Fn(&gpui::MouseDownEvent, &mut Window, &mut gpui::App) + 'static {
        let target = self
            .selected_channel
            .zip(self.selected_channel.and_then(|c| self.history.get(c)?.messages.get(ix)).map(|m| m.id));
        cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
            if let Some((channel, message)) = target {
                this.open_message_menu(channel, message, event.position, cx);
            }
        })
    }

    pub(in super::super) fn render_message_menu(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let menu = self.message_menu.as_ref()?;
        let (channel, id) = (menu.channel, menu.message);
        let (_, msg) = self.history.find(channel, id)?;
        let rights = self.rights(channel, msg);
        let item = |name: &'static str, label: &'static str, glyph: &'static str, danger: bool| {
            let color = if danger { theme::DANGER } else { theme::INTERACTIVE };
            div()
                .id(name)
                .h(px(32.))
                .px_2()
                .flex()
                .items_center()
                .gap_2()
                .rounded_md()
                .cursor_pointer()
                .text_size(px(14.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(if danger { theme::DANGER } else { theme::TEXT_DEFAULT }))
                .hover(move |s| s.bg(if danger { rgb(theme::BUTTON_DANGER) } else { theme::hover() }))
                .child(div().flex_1().child(label))
                .child(icon(glyph, 18., color))
        };
        let pin_label = if msg.pinned { t!("message.unpin") } else { t!("message.pin") };
        let react = self.may_react(channel, msg, true);
        let thread = self.may_start_thread(channel, msg);
        let position = menu.position;
        let list = div()
            .id("message-menu")
            .w(px(220.))
            .flex()
            .flex_col()
            .p_2()
            .rounded_lg()
            .border_1()
            .border_color(theme::border())
            .bg(rgb(theme::SURFACE_HIGH))
            .shadow_lg()
            .occlude()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.message_menu = None;
                cx.notify();
            }))
            .when(react, |menu| {
                menu.child(item("menu-react", t!("reaction.add"), icons::REACTION, false).on_click(cx.listener(
                    move |this, _, window, cx| this.open_reaction_picker(channel, id, position, window, cx),
                )))
            })
            .when(rights.reply, |menu| {
                menu.child(item("menu-reply", t!("message.reply"), icons::REPLY, false).on_click(cx.listener(
                    move |this, _, window, cx| this.start_reply(channel, id, window, cx),
                )))
            })
            .when(thread, |menu| {
                menu.child(item("menu-thread", t!("thread.create"), icons::THREAD, false).on_click(cx.listener(
                    move |this, _, window, cx| this.open_create_thread(channel, Some(id), window, cx),
                )))
            })
            .when(rights.edit, |menu| {
                menu.child(item("menu-edit", t!("message.edit"), icons::EDIT, false).on_click(cx.listener(
                    move |this, _, window, cx| this.start_edit(channel, id, window, cx),
                )))
            })
            .when(rights.pin, |menu| {
                menu.child(item("menu-pin", pin_label, icons::PIN, false).on_click(cx.listener(
                    move |this, _, _, cx| this.toggle_pin(channel, id, cx),
                )))
            })
            .when(!msg.content.is_empty(), |menu| {
                menu.child(item("menu-copy", t!("message.copy_text"), icons::COPY, false).on_click(cx.listener(
                    move |this, _, _, cx| this.copy_message(channel, id, false, cx),
                )))
            })
            .when(rights.delete, |menu| {
                menu.child(item("menu-delete", t!("message.delete"), icons::TRASH, true).on_click(cx.listener(
                    move |this, event: &gpui::ClickEvent, _, cx| {
                        this.ask_delete(channel, id, event.modifiers().shift, cx)
                    },
                )))
            })
            .child(div().my_1().h(px(1.)).bg(theme::border()))
            .child(item("menu-copy-id", t!("message.copy_id"), icons::COPY, false).on_click(cx.listener(
                move |this, _, _, cx| this.copy_message(channel, id, true, cx),
            )));
        Some(
            deferred(anchored().position(menu.position).snap_to_window_with_margin(px(8.)).child(list))
                .with_priority(1)
                .into_any_element(),
        )
    }

    /// "Delete Message": the message as it will vanish, then the choice.
    pub(in super::super) fn render_delete_modal(&self, channel: Snowflake, id: Snowflake, cx: &mut Context<Self>) -> impl IntoElement {
        let preview = self.history.find(channel, id).map(|(_, msg)| {
            let (author, color) = self.author_style(msg);
            div()
                .flex()
                .gap_3()
                .p_3()
                .rounded_lg()
                .border_1()
                .border_color(theme::border())
                .bg(rgb(theme::BASE_LOWER))
                .child(widgets::avatar(&author, msg.author_id, self.avatar_image(&msg.avatar_url), 40., None))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w(px(0.))
                        .child(
                            div()
                                .flex()
                                .gap(px(6.))
                                .items_center()
                                .child(div().text_color(rgb(color)).font_weight(FontWeight::MEDIUM).child(author))
                                .child(div().text_xs().text_color(theme::muted()).child(msg.time.clone())),
                        )
                        .child(self.rich_text("delete-preview", &msg.content, &msg.mentions, false)),
                )
        });
        div()
            .id("delete-scrim")
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
                    .id("delete-modal")
                    .w(px(440.))
                    .rounded_lg()
                    .overflow_hidden()
                    .border_1()
                    .border_color(theme::border())
                    .bg(rgb(theme::SURFACE_HIGH))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .p_4()
                            .child(
                                div()
                                    .text_size(px(20.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(theme::TEXT_STRONG))
                                    .child(t!("message.delete")),
                            )
                            .child(div().text_color(rgb(theme::TEXT_DEFAULT)).child(t!("message.delete_confirm")))
                            .children(preview)
                            .child(div().text_xs().text_color(theme::muted()).child(t!("message.delete_tip"))),
                    )
                    .child(
                        div()
                            .p_4()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .bg(rgb(theme::BASE_LOW))
                            .child(
                                widgets::button("delete-cancel", t!("common.cancel"), widgets::ButtonKind::Secondary)
                                    .on_click(cx.listener(|this, _, window, cx| this.close_overlay(window, cx))),
                            )
                            .child(
                                widgets::button("delete-confirm", t!("message.delete_button"), widgets::ButtonKind::Danger)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.close_overlay(window, cx);
                                        this.delete_message(channel, id, cx);
                                    })),
                            ),
                    ),
            )
    }

    /// The edit box in place of a message's text.
    pub(super) fn render_edit_box(&self) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .py_1()
            .child(self.edit_input.clone())
            .child(div().text_xs().text_color(theme::muted()).child(t!("message.edit_hint")))
    }

    /// "Replying to …" above the message box, with the mention switch.
    pub(super) fn render_reply_bar(&self, cx: &mut Context<Self>) -> Option<Div> {
        let reply = self.composer.reply.as_ref().filter(|r| Some(r.channel) == self.selected_channel)?;
        let (text, names) = t_spans!("message.replying_to", name = reply.name);
        let mention = reply.mention;
        let mine = self.people.is_me(reply.preview.author_id);
        Some(
            div()
                .flex()
                .items_center()
                .gap_2()
                .h(px(34.))
                .px_4()
                .rounded_t_lg()
                .bg(rgb(theme::BASE_LOW))
                .text_size(px(14.))
                .text_color(rgb(theme::TEXT_MUTED))
                .child(div().flex_1().min_w(px(0.)).truncate().child(gpui::StyledText::new(text).with_highlights(
                    names.into_iter().map(|range| {
                        (
                            range,
                            gpui::HighlightStyle {
                                font_weight: Some(FontWeight::SEMIBOLD),
                                color: Some(rgb(theme::TEXT_DEFAULT).into()),
                                ..Default::default()
                            },
                        )
                    }),
                )))
                .when(!mine, |bar| {
                    bar.child(
                        div()
                            .id("reply-mention")
                            .flex_none()
                            .cursor_pointer()
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(12.))
                            .text_color(rgb(if mention { theme::LINK } else { theme::TEXT_MUTED }))
                            .child(format!(
                                "@ {}",
                                if mention { t!("message.mention_on") } else { t!("message.mention_off") }
                            ))
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(reply) = &mut this.composer.reply {
                                    reply.mention = !reply.mention;
                                }
                                cx.notify();
                            })),
                    )
                })
                .child(div().w(px(1.)).h(px(16.)).bg(theme::border()))
                .child(
                    div()
                        .id("reply-cancel")
                        .flex_none()
                        .size(px(18.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .cursor_pointer()
                        .bg(rgb(theme::INTERACTIVE))
                        .hover(|s| s.bg(rgb(theme::TEXT_STRONG)))
                        .text_size(px(10.))
                        .text_color(rgb(theme::BASE_LOW))
                        .child("✕")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.composer.reply = None;
                            cx.notify();
                        })),
                ),
        )
    }
}
