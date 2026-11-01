//! "Set Custom Reminder": a date and a time typed by hand, for the
//! reminder of one message.

use gpui::{AnyElement, Context, Entity, Focusable, FontWeight, MouseButton, Window, div, prelude::*, rgb};

use super::super::text_input::TextInput;
use super::super::theme;
use super::state::remind;
use super::widgets::{self, ButtonKind};
use super::{Emerald, Overlay};
use crate::time;
use crate::zoom::zoomed;
use emerald_discord::SaveSource;
use emerald_discord::model::Snowflake;

pub(super) struct CustomReminder {
    channel: Snowflake,
    message: Snowflake,
    date: Entity<TextInput>,
    time: Entity<TextInput>,
    /// The last try did not read as a future time.
    invalid: bool,
}

/// Whether the language writes the day before the month.
fn day_first() -> bool {
    t!("time.numeric_date").starts_with("{day}")
}

impl Emerald {
    pub(super) fn open_custom_reminder(&mut self, channel: Snowflake, message: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        self.message_menu = None;
        let hint = t!("time.numeric_date", day = "DD", month = "MM", year = "YYYY");
        let date = cx.new(|cx| TextInput::new(hint, cx).keep_on_submit().compact());
        let time = cx.new(|cx| TextInput::new(t!("time.clock_24", hour = "HH", minute = "MM"), cx).keep_on_submit().compact());
        window.focus(&date.focus_handle(cx));
        self.overlay = Some(Overlay::CustomReminder(Box::new(CustomReminder {
            channel,
            message,
            date,
            time,
            invalid: false,
        })));
        cx.notify();
    }

    fn save_custom_reminder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Overlay::CustomReminder(form)) = &mut self.overlay else { return };
        let date = form.date.read(cx).text().to_owned();
        let clock = form.time.read(cx).text().to_owned();
        let Some(due) = remind::parse_custom(&date, &clock, day_first(), time::now_secs()) else {
            form.invalid = true;
            cx.notify();
            return;
        };
        let (channel, message) = (form.channel, form.message);
        self.close_overlay(window, cx);
        self.set_reminder(channel, message, due, SaveSource::ContextMenu);
    }

    pub(super) fn render_custom_reminder(&self, form: &CustomReminder, cx: &mut Context<Self>) -> AnyElement {
        let field = |label: &'static str, input: Entity<TextInput>| {
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_size(zoomed(12.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(theme::text_default()))
                        .child(label),
                )
                .child(
                    div()
                        .px_3()
                        .py_2()
                        .rounded(theme::radius::control())
                        .border_1()
                        .border_color(theme::border_normal())
                        .bg(rgb(theme::base_lowest()))
                        .child(input),
                )
        };
        div()
            .id("reminder-scrim")
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
                    .id("reminder-modal")
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
                                    .child(t!("saved.custom_title")),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_3()
                                    .child(field(t!("saved.date"), form.date.clone()))
                                    .child(field(t!("saved.time"), form.time.clone())),
                            )
                            .when(form.invalid, |body| {
                                body.child(
                                    div()
                                        .text_size(zoomed(12.))
                                        .text_color(rgb(theme::danger()))
                                        .child(t!("saved.custom_invalid")),
                                )
                            }),
                    )
                    .child(
                        div()
                            .p_4()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .bg(rgb(theme::base_low()))
                            .child(
                                widgets::button("reminder-cancel", t!("common.cancel"), ButtonKind::Secondary)
                                    .on_click(cx.listener(|this, _, window, cx| this.close_overlay(window, cx))),
                            )
                            .child(
                                widgets::button("reminder-save", t!("common.save"), ButtonKind::Brand)
                                    .on_click(cx.listener(|this, _, window, cx| this.save_custom_reminder(window, cx))),
                            ),
                    ),
            )
            .into_any_element()
    }
}
