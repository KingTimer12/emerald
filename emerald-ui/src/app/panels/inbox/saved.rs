//! The Inbox's Bookmarks and Reminders tabs: one flat list each, as the
//! web client draws them, reminders soonest (and overdue) first.

use super::*;
use super::super::super::state::remind::{self, RemindAt};
use super::super::super::state::saved::Entry;
use crate::time;
use emerald_discord::SaveSource;

impl Emerald {
    /// The tab's list, its "Load more" for bookmarks, or its empty state.
    pub(super) fn render_saved(&self, inbox: &Inbox, reminders: bool, cx: &mut Context<Self>) -> AnyElement {
        let entries = if reminders { self.saved.reminders() } else { self.saved.bookmarks() };
        let now = time::now_secs();
        let more = (!reminders && self.saved.has_more_bookmarks()).then(|| {
            div().p_3().flex().justify_center().child(
                widgets::link_button("bookmarks-more", t!("saved.load_more")).on_click(cx.listener(|this, _, _, cx| {
                    this.more_bookmarks();
                    cx.notify();
                })),
            )
        });
        let empty = entries.is_empty().then(|| {
            let (title, body, item) = if reminders {
                ("saved.empty_reminders_title", "saved.empty_reminders_body", t!("saved.create_reminder"))
            } else {
                ("saved.empty_bookmarks_title", "saved.empty_bookmarks_body", t!("saved.bookmark"))
            };
            div()
                .p_6()
                .flex()
                .flex_col()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .text_size(zoomed(16.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(theme::text_strong()))
                        .child(emerald_core::i18n::text(title)),
                )
                .child(
                    div()
                        .text_size(zoomed(14.))
                        .text_color(theme::muted())
                        .text_center()
                        .child(emerald_core::i18n::format(body, &[("item", &item as &dyn std::fmt::Display)])),
                )
        });
        div()
            .id(if reminders { "inbox-reminders" } else { "inbox-bookmarks" })
            .flex_1()
            .min_h(zoomed(0.))
            .overflow_y_scroll()
            .p_2()
            .flex()
            .flex_col()
            .gap_2()
            .children(
                entries
                    .iter()
                    .enumerate()
                    .map(|(ix, entry)| self.render_saved_row(inbox, entry, ix, reminders, now, cx)),
            )
            .children(more)
            .children(empty)
            .into_any_element()
    }

    fn render_saved_row(&self, inbox: &Inbox, entry: &Entry, ix: usize, reminders: bool, now: i64, cx: &mut Context<Self>) -> AnyElement {
        let data = &entry.data;
        let (guild, channel, message) = (data.guild_id, data.channel_id, data.message_id);
        let editing = inbox.editing == Some(message);
        let due = entry.due_secs();
        let chip = due.filter(|_| reminders).map(|due| {
            let overdue = due <= now;
            let text = if overdue {
                t!("saved.due_ago", duration = time::duration((now - due) as u64))
            } else {
                t!("saved.due_in", duration = time::duration((due - now) as u64))
            };
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap_1()
                .px_2()
                .h(zoomed(20.))
                .rounded(theme::radius::pill())
                .bg(if overdue { rgb(theme::danger()) } else { theme::hover() })
                .text_size(zoomed(12.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(if overdue { theme::on_color() } else { theme::text_strong() }))
                .child(icons::icon(icons::CLOCK, 12., if overdue { theme::on_color() } else { theme::text_strong() }))
                .child(text)
        });
        let action = |id: &'static str, glyph: &'static str, tip: &'static str| {
            square_button((id, ix), glyph).tooltip(widgets::tooltip(tip))
        };
        let actions = div()
            .flex_none()
            .flex()
            .gap_1()
            .when(reminders, |row| {
                let tip = if due.is_some_and(|due| due <= now) { t!("saved.snooze") } else { t!("saved.edit_reminder") };
                row.child(action("saved-edit", icons::EDIT, tip).on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    if let Some(inbox) = &mut this.inbox {
                        inbox.editing = (inbox.editing != Some(message)).then_some(message);
                    }
                    cx.notify();
                })))
            })
            .when(entry.msg.is_some(), |row| {
                row.child(action("saved-jump", icons::ARROW_RIGHT, t!("saved.jump")).on_click(cx.listener(
                    move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.close_inbox(cx);
                        this.open_channel_link(guild, channel, Some(message), window, cx);
                    },
                )))
            })
            .child({
                let tip = if reminders { t!("saved.remove_reminder") } else { t!("saved.remove_bookmark") };
                action("saved-remove", icons::CLOSE, tip).on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.unsave(channel, message);
                    cx.notify();
                }))
            });
        let header = div()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .pt_2()
            .child(
                div()
                    .flex_1()
                    .min_w(zoomed(0.))
                    .truncate()
                    .text_size(zoomed(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::muted())
                    .child(self.place_label(guild, channel)),
            )
            .children(chip)
            .child(actions);
        let body = match &entry.msg {
            Some(msg) => {
                let (author, color) = self.author_style(msg);
                div()
                    .px_3()
                    .pb_2()
                    .flex()
                    .gap_3()
                    .child(widgets::avatar(&author, msg.author_id, self.avatar_image(&msg.avatar_url), 40., None))
                    .child(
                        div()
                            .flex_1()
                            .min_w(zoomed(0.))
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .flex()
                                    .items_baseline()
                                    .gap_2()
                                    .child(div().font_weight(FontWeight::MEDIUM).text_color(rgb(color)).child(author))
                                    .child(div().text_xs().text_color(rgb(theme::text_muted())).child(msg.time.clone())),
                            )
                            .child(
                                div()
                                    .text_color(rgb(theme::text_default()))
                                    .child(self.rich_text(("saved-text", ix), &msg.content, &msg.mentions)),
                            ),
                    )
            }
            None => div().px_3().pb_2().text_size(zoomed(14.)).text_color(theme::muted()).child(if reminders {
                t!("saved.reminder_missing")
            } else {
                t!("saved.bookmark_missing")
            }),
        };
        let choices = if editing { Some(self.render_remind_choices(channel, message, ix, now, cx)) } else { None };
        div()
            .id(("saved-row", ix))
            .flex()
            .flex_col()
            .gap_1()
            .rounded(theme::radius::card())
            .border_1()
            .border_color(theme::border_normal())
            .bg(rgb(theme::base_low()))
            .when(entry.msg.is_some(), |row| {
                row.cursor_pointer().on_click(cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                    // Shift keeps the Inbox open, as in the web client.
                    if !event.modifiers().shift {
                        this.close_inbox(cx);
                    }
                    this.open_channel_link(guild, channel, Some(message), window, cx);
                }))
            })
            .child(header)
            .child(body)
            .children(choices)
            .into_any_element()
    }

    /// The due times offered to edit or snooze a reminder, in a row.
    fn render_remind_choices(&self, channel: Snowflake, message: Snowflake, ix: usize, now: i64, cx: &mut Context<Self>) -> AnyElement {
        let day = time::day_time(now);
        div()
            .px_3()
            .pb_2()
            .flex()
            .flex_wrap()
            .gap_1()
            .children(RemindAt::ALL.into_iter().filter_map(|choice| {
                let day = day?;
                let due = remind::remind_at(choice, now, day);
                Some(
                    widgets::chip(("saved-at", ix * 8 + choice as usize), emerald_core::i18n::text(choice.label(day)), false)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            if let Some(inbox) = &mut this.inbox {
                                inbox.editing = None;
                            }
                            this.set_reminder(channel, message, due, SaveSource::ForLaterList);
                            cx.notify();
                        })),
                )
            }))
            .child(
                widgets::chip(("saved-at", ix * 8 + 7), t!("saved.custom"), false).on_click(cx.listener(
                    move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.close_inbox(cx);
                        this.open_custom_reminder(channel, message, window, cx);
                    },
                )),
            )
            .into_any_element()
    }
}
