//! The hover toolbar, the context menu and the delete confirmation.

use super::*;
use super::bulk;
use crate::zoom::zoomed;

impl Emerald {
    pub(super) fn open_message_menu(&mut self, channel: Snowflake, message: Snowflake, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.message_menu = Some(MessageMenu {
            channel,
            message,
            position,
        });
        cx.notify();
    }

    /// Deletes at once with shift held, else asks first, like Discord.
    pub(super) fn ask_delete(&mut self, channel: Snowflake, message: Snowflake, shift: bool, cx: &mut Context<Self>) {
        self.message_menu = None;
        if shift {
            self.delete_message(channel, message, cx);
        } else {
            self.overlay = Some(Overlay::DeleteMessage { channel, message });
            cx.notify();
        }
    }

    pub(super) fn delete_message(&mut self, channel: Snowflake, message: Snowflake, cx: &mut Context<Self>) {
        self.net.post(Command::DeleteMessage { channel, message });
        cx.notify();
    }

    pub(super) fn toggle_pin(&mut self, channel: Snowflake, message: Snowflake, cx: &mut Context<Self>) {
        self.message_menu = None;
        let Some((_, msg)) = self.history.find(channel, message) else { return };
        let pin = !msg.pinned;
        self.net.post(Command::PinMessage { channel, message, pin });
        cx.notify();
    }

    pub(super) fn copy_message(&mut self, channel: Snowflake, message: Snowflake, id: bool, cx: &mut Context<Self>) {
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
}

impl Emerald {
    /// The hover toolbar in the top right corner of a message.
    pub(in super::super) fn message_toolbar(&self, msg: &Msg, ix: usize, group: SharedString, cx: &mut Context<Self>) -> Option<AnyElement> {
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
                .size(zoomed(32.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(theme::radius::control())
                .cursor_pointer()
                .hover(|s| s.bg(theme::hover()))
                .tooltip(widgets::tooltip(tip))
                .child(icon(glyph, 20., theme::interactive()))
        };
        Some(
            div()
                .absolute()
                .top(zoomed(-16.))
                .right(zoomed(14.))
                .flex()
                .p(zoomed(2.))
                .rounded(theme::radius::control())
                .border_1()
                .border_color(theme::border())
                .bg(rgb(theme::base_lower()))
                .when(!open, |bar| bar.invisible().group_hover(group, |s| s.visible()))
                .children(quick.into_iter().enumerate().map(|(n, emoji)| {
                    let label = emoji_label(&emoji);
                    div()
                        .id(("msg-quick", ix * QUICK_REACTIONS + n))
                        .size(zoomed(32.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(theme::radius::control())
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
    pub(in super::super) fn on_message_right_click(&self, ix: usize, cx: &mut Context<Self>) -> impl Fn(&gpui::MouseDownEvent, &mut Window, &mut gpui::App) + 'static {
        let target = self
            .selected_channel
            .zip(self.selected_channel.and_then(|c| self.history.get(c)?.messages.get(ix)).map(|m| m.id));
        cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
            if let Some((channel, message)) = target {
                this.open_message_menu(channel, message, event.position, cx);
            }
        })
    }

    pub(in super::super::super) fn render_message_menu(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let menu = self.message_menu.as_ref()?;
        let (channel, id) = (menu.channel, menu.message);
        let (_, msg) = self.history.find(channel, id)?;
        let rights = self.rights(channel, msg);
        let item = |name: &'static str, label: &'static str, glyph: &'static str, danger: bool| {
            let color = if danger { theme::danger() } else { theme::interactive() };
            div()
                .id(name)
                .h(zoomed(32.))
                .px_2()
                .flex()
                .items_center()
                .gap_2()
                .rounded(theme::radius::control())
                .cursor_pointer()
                .text_size(zoomed(14.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(if danger { theme::danger() } else { theme::text_default() }))
                .hover(move |s| s.bg(if danger { rgb(theme::button_danger()) } else { theme::hover() }))
                .child(div().flex_1().child(label))
                .child(icon(glyph, 18., color))
        };
        let pin_label = if msg.pinned { t!("message.unpin") } else { t!("message.pin") };
        let react = self.may_react(channel, msg, true);
        let thread = self.may_start_thread(channel, msg);
        let this_picked = self.is_bulk_picked(channel, id);
        let position = menu.position;
        let list = div()
            .id("message-menu")
            .w(zoomed(220.))
            .flex()
            .flex_col()
            .p_2()
            .rounded(theme::radius::modal())
            .border_1()
            .border_color(theme::border())
            .bg(rgb(theme::surface_high()))
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
            .when(rights.delete && bulk::deletable(id), |menu| {
                let label = if this_picked { t!("bulk.unselect") } else { t!("bulk.select") };
                menu.child(item("menu-bulk", label, icons::CHECK, false).on_click(cx.listener(
                    move |this, _, _, cx| this.toggle_bulk(channel, id, cx),
                )))
            })
            .when(rights.delete, |menu| {
                menu.child(item("menu-delete", t!("message.delete"), icons::TRASH, true).on_click(cx.listener(
                    move |this, event: &gpui::ClickEvent, _, cx| {
                        this.ask_delete(channel, id, event.modifiers().shift, cx)
                    },
                )))
            })
            .child(div().my_1().h(zoomed(1.)).bg(theme::border()))
            .child(item("menu-copy-id", t!("message.copy_id"), icons::COPY, false).on_click(cx.listener(
                move |this, _, _, cx| this.copy_message(channel, id, true, cx),
            )));
        Some(
            deferred(anchored().position(menu.position).snap_to_window_with_margin(zoomed(8.)).child(list))
                .with_priority(1)
                .into_any_element(),
        )
    }

    /// "Delete Message": the message as it will vanish, then the choice.
    pub(in super::super::super) fn render_delete_modal(&self, channel: Snowflake, id: Snowflake, cx: &mut Context<Self>) -> impl IntoElement {
        let preview = self.history.find(channel, id).map(|(_, msg)| {
            let (author, color) = self.author_style(msg);
            div()
                .flex()
                .gap_3()
                .p_3()
                .rounded(theme::radius::card())
                .border_1()
                .border_color(theme::border())
                .bg(rgb(theme::base_lower()))
                .child(widgets::avatar(&author, msg.author_id, self.avatar_image(&msg.avatar_url), 40., None))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w(zoomed(0.))
                        .child(
                            div()
                                .flex()
                                .gap(zoomed(6.))
                                .items_center()
                                .child(div().text_color(rgb(color)).font_weight(FontWeight::MEDIUM).child(author))
                                .child(div().text_xs().text_color(theme::muted()).child(msg.time.clone())),
                        )
                        .child(self.rich_text("delete-preview", &msg.content, &msg.mentions)),
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
                                    .child(t!("message.delete")),
                            )
                            .child(div().text_color(rgb(theme::text_default())).child(t!("message.delete_confirm")))
                            .children(preview)
                            .child(div().text_xs().text_color(theme::muted()).child(t!("message.delete_tip"))),
                    )
                    .child(
                        div()
                            .p_4()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .bg(rgb(theme::base_low()))
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
}
