//! The context menu and the delete confirmation.

use super::*;
use super::bulk;
use super::super::super::state::hover_bar;
use super::super::super::state::remind::{self, RemindAt};
use super::super::widgets::popover::{self, Anchor, Tracked};
use crate::time;
use crate::zoom::zoomed;
use emerald_discord::SaveSource;

/// Width of the menu and of its "Remind Me" side menu.
const MENU_WIDTH: f32 = 220.;

impl Emerald {
    pub(super) fn open_message_menu(&mut self, channel: Snowflake, message: Snowflake, anchor: Anchor, cx: &mut Context<Self>) {
        self.message_menu = Some(MessageMenu {
            channel,
            message,
            anchor,
            panel: Tracked::default(),
            remind: false,
            remind_item: Tracked::default(),
            remind_panel: Tracked::default(),
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

    /// "Copy Message Link": the discord.com link of the message.
    pub(super) fn copy_message_link(&mut self, channel: Snowflake, message: Snowflake, cx: &mut Context<Self>) {
        self.message_menu = None;
        let space = self.spaces.current();
        let guild = (!space.is_dms()).then_some(space.id);
        cx.write_to_clipboard(ClipboardItem::new_string(hover_bar::message_link(guild, channel, message)));
        cx.notify();
    }

    /// "Mark Unread": the read marker goes back to just before `message`,
    /// with the loaded mentions after it still waiting. Threads only when
    /// already joined and open, since joining or unarchiving is a write.
    pub(super) fn mark_unread_from(&mut self, channel: Snowflake, message: Snowflake, cx: &mut Context<Self>) {
        self.message_menu = None;
        let Some(log) = self.history.get(channel) else { return };
        let loaded: Vec<Snowflake> = log.messages.iter().filter(|m| !m.pending).map(|m| m.id).collect();
        let before = hover_bar::unread_from(&loaded, message);
        let me = self.people.my_id();
        let my_roles = self.spaces.current().info().map_or(&[][..], |g| g.my_roles());
        let mentions = log
            .messages
            .iter()
            .filter(|m| m.id > before && m.author_id != me && m.mentions_user(me, my_roles))
            .count() as u32;
        self.reads.marked_unread(channel, before, mentions);
        self.net.post(Command::MarkUnread { channel, before, mentions });
        cx.notify();
    }

    /// Whether "Mark Unread" can be offered in `channel`.
    pub(super) fn may_mark_unread(&self, channel: Snowflake) -> bool {
        self.spaces
            .channel(channel)
            .is_none_or(|c| !c.kind.is_thread() || (c.member.is_some() && c.is_active_thread()))
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
    /// Right click on a message opens its menu where the pointer is.
    pub(in super::super) fn on_message_right_click(&self, ix: usize, cx: &mut Context<Self>) -> impl Fn(&gpui::MouseDownEvent, &mut Window, &mut gpui::App) + 'static {
        let target = self
            .selected_channel
            .zip(self.selected_channel.and_then(|c| self.history.get(c)?.messages.get(ix)).map(|m| m.id));
        cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
            if let Some((channel, message)) = target {
                this.open_message_menu(channel, message, Anchor::pointer(event.position), cx);
            }
        })
    }

    pub(in super::super::super) fn render_message_menu(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let menu = self.message_menu.as_ref()?;
        let (channel, id) = (menu.channel, menu.message);
        let (_, msg) = self.history.find(channel, id)?;
        let rights = self.rights(channel, msg);
        let pin_label = if msg.pinned { t!("message.unpin") } else { t!("message.pin") };
        let react = self.may_react(channel, msg, true);
        let thread = self.may_start_thread(channel, msg);
        let forward = matches!(msg.kind, message_kind::DEFAULT | message_kind::REPLY);
        let this_picked = self.is_bulk_picked(channel, id);
        let reminder = self.saved.reminder(id).is_some();
        let bookmarked = self.saved.is_bookmarked(id);
        let anchor = menu.anchor;
        let separator = || div().my_1().h(zoomed(1.)).bg(theme::border());
        let remind_label = if reminder { t!("saved.edit_reminder") } else { t!("saved.create_reminder") };
        let list = menu.panel.track(div())
            .id("message-menu")
            .relative()
            .w(zoomed(MENU_WIDTH))
            .flex()
            .flex_col()
            .p_2()
            .rounded(theme::radius::modal())
            .border_1()
            .border_color(theme::border())
            .bg(rgb(theme::surface_high()))
            .shadow_lg()
            .occlude()
            .on_mouse_down_out(cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                // The side menu lies outside this box; presses there are its own.
                if let Some(menu) = &this.message_menu
                    && menu.remind
                    && menu.remind_panel.bounds().contains(&event.position)
                {
                    return;
                }
                this.message_menu = None;
                cx.notify();
            }))
            .when(react, |menu| {
                menu.child(menu_item("menu-react", t!("reaction.add"), icons::REACTION, false).on_click(cx.listener(
                    move |this, _, window, cx| this.open_reaction_picker(channel, id, anchor, window, cx),
                )))
                .child(separator())
            })
            .when(rights.edit, |menu| {
                menu.child(menu_item("menu-edit", t!("message.edit"), icons::EDIT, false).on_click(cx.listener(
                    move |this, _, window, cx| this.start_edit(channel, id, window, cx),
                )))
            })
            .when(rights.reply, |menu| {
                menu.child(menu_item("menu-reply", t!("message.reply"), icons::REPLY, false).on_click(cx.listener(
                    move |this, _, window, cx| this.start_reply(channel, id, window, cx),
                )))
            })
            .when(forward, |menu| {
                menu.child(menu_item("menu-forward", t!("forward.action"), icons::FORWARD, false).on_click(cx.listener(
                    move |this, _, window, cx| this.open_forward(channel, id, window, cx),
                )))
            })
            .when(thread, |menu| {
                menu.child(menu_item("menu-thread", t!("thread.create"), icons::THREAD, false).on_click(cx.listener(
                    move |this, _, window, cx| this.open_create_thread(channel, Some(id), window, cx),
                )))
            })
            .child(separator())
            .when(!msg.content.is_empty(), |menu| {
                menu.child(menu_item("menu-copy", t!("message.copy_text"), icons::COPY, false).on_click(cx.listener(
                    move |this, _, _, cx| this.copy_message(channel, id, false, cx),
                )))
            })
            .when(rights.pin, |menu| {
                menu.child(menu_item("menu-pin", pin_label, icons::PIN, false).on_click(cx.listener(
                    move |this, _, _, cx| this.toggle_pin(channel, id, cx),
                )))
            })
            .when(!reminder, |menu| {
                let (label, glyph) = if bookmarked {
                    (t!("saved.remove_bookmark"), icons::BOOKMARK_FILLED)
                } else {
                    (t!("saved.bookmark"), icons::BOOKMARK)
                };
                menu.child(menu_item("menu-bookmark", label, glyph, false).on_click(cx.listener(move |this, _, _, cx| {
                    this.message_menu = None;
                    this.toggle_bookmark(channel, id, SaveSource::ContextMenu);
                    cx.notify();
                })))
            })
            .child(
                menu.remind_item
                    .track(menu_item("menu-remind", remind_label, icons::CHEVRON_RIGHT, false))
                    .when(menu.remind, |item| item.bg(theme::hover()))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(menu) = &mut this.message_menu {
                            menu.remind = !menu.remind;
                        }
                        cx.notify();
                    })),
            )
            .when(self.may_mark_unread(channel), |menu| {
                menu.child(menu_item("menu-unread", t!("message.mark_unread"), icons::MARK_UNREAD, false).on_click(
                    cx.listener(move |this, _, _, cx| this.mark_unread_from(channel, id, cx)),
                ))
            })
            .child(menu_item("menu-link", t!("message.copy_message_link"), icons::LINK, false).on_click(cx.listener(
                move |this, _, _, cx| this.copy_message_link(channel, id, cx),
            )))
            .when(rights.delete && bulk::deletable(id), |menu| {
                let label = if this_picked { t!("bulk.unselect") } else { t!("bulk.select") };
                menu.child(menu_item("menu-bulk", label, icons::CHECK, false).on_click(cx.listener(
                    move |this, _, _, cx| this.toggle_bulk(channel, id, cx),
                )))
            })
            .when(rights.delete, |menu| {
                menu.child(separator()).child(
                    menu_item("menu-delete", t!("message.delete"), icons::TRASH, true).on_click(cx.listener(
                        move |this, event: &gpui::ClickEvent, _, cx| this.ask_delete(channel, id, event.modifiers().shift, cx),
                    )),
                )
            })
            .child(separator())
            .child(menu_item("menu-copy-id", t!("message.copy_id"), icons::COPY, false).on_click(cx.listener(
                move |this, _, _, cx| this.copy_message(channel, id, true, cx),
            )))
            .when(menu.remind, |list| list.child(self.remind_menu(menu, reminder, cx)));
        Some(popover::at(menu.anchor, list))
    }

    /// "Remind Me": the due times beside the menu, and for a message with a
    /// reminder, "Mark Complete" above them.
    fn remind_menu(&self, menu: &MessageMenu, reminder: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let (channel, id) = (menu.channel, menu.message);
        let now = time::now_secs();
        let day = time::day_time(now);
        let heading = |text: SharedString| {
            div()
                .px_2()
                .py_1()
                .text_size(zoomed(12.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::muted())
                .child(text)
        };
        let due_text = self.saved.reminder(id).and_then(|s| s.due_secs()).map(|due| {
            if due > now {
                t!("saved.due_in_long", duration = time::duration((due - now) as u64))
            } else {
                t!("saved.due_ago_long", duration = time::duration((now - due) as u64))
            }
        });
        let choices = RemindAt::ALL.into_iter().filter_map(|choice| {
            let day = day?;
            let due = remind::remind_at(choice, now, day);
            Some(
                menu_item(("remind-at", choice as usize), emerald_core::i18n::text(choice.label(day)), "", false).on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.message_menu = None;
                        this.set_reminder(channel, id, due, SaveSource::ContextMenu);
                        cx.notify();
                    },
                )),
            )
        });
        // Level with its item, measured from the menu's top.
        let offset = (menu.remind_item.bounds().top() - menu.panel.bounds().top()).max(gpui::Pixels::ZERO);
        menu.remind_panel
            .track(popover::panel("remind-menu", MENU_WIDTH))
            .absolute()
            .left(zoomed(MENU_WIDTH + 4.))
            .top(offset)
            .when_some(due_text.filter(|_| reminder), |panel, text| {
                panel
                    .child(heading(text.into()))
                    .child(menu_item("remind-complete", t!("saved.mark_complete"), icons::CHECK, false).on_click(
                        cx.listener(move |this, _, _, cx| {
                            this.message_menu = None;
                            this.unsave(channel, id);
                            cx.notify();
                        }),
                    ))
                    .child(div().my_1().h(zoomed(1.)).bg(theme::border()))
            })
            .child(heading(if reminder { t!("saved.edit_reminder") } else { t!("saved.remind_me") }.into()))
            .children(choices)
            .child(menu_item("remind-custom", t!("saved.custom"), "", false).on_click(cx.listener(
                move |this, _, window, cx| this.open_custom_reminder(channel, id, window, cx),
            )))
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

/// A row of the message menu: its label, then its icon on the right.
fn menu_item(id: impl Into<gpui::ElementId>, label: impl Into<SharedString>, glyph: &'static str, danger: bool) -> gpui::Stateful<Div> {
    let color = if danger { theme::danger() } else { theme::interactive() };
    div()
        .id(id)
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
        .child(div().flex_1().child(label.into()))
        .when(!glyph.is_empty(), |row| row.child(icon(glyph, 18., color)))
}
