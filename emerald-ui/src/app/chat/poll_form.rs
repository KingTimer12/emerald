//! "Create Poll": the question, up to ten answers, how long it runs and
//! whether several answers may be picked, with the web client's limits.
//! Opened from the + beside the message box, which also uploads files.

use gpui::{AnyElement, Context, Entity, Focusable, FontWeight, SharedString, Window, div, prelude::*, rgb};

use super::super::super::text_input::TextInput;
use super::super::super::{icons, theme, time};
use super::super::{Emerald, Overlay, widgets};
use emerald_discord::model::Snowflake;
use emerald_discord::{Action, Command, NewPoll as PollToSend, permissions, rest};
use crate::zoom::zoomed;

/// The web client's limits: question and answer lengths, and answers.
const MAX_QUESTION: usize = 300;
const MAX_ANSWER: usize = 55;
const MAX_ANSWERS: usize = 10;
/// The durations it offers, in hours, and the one it starts on.
const DURATIONS: [u32; 7] = [1, 4, 8, 24, 72, 168, 336];
const DEFAULT_HOURS: u32 = 24;

/// "Create Poll" for `channel`.
pub(in super::super) struct NewPoll {
    channel: Snowflake,
    question: Entity<TextInput>,
    answers: Vec<Entity<TextInput>>,
    hours: u32,
    multiple: bool,
    pub error: Option<SharedString>,
}

impl Emerald {
    /// Polls go where messages go, in guilds with the right to post them.
    fn can_poll(&self, channel: Snowflake) -> bool {
        let space = self.spaces.current();
        self.can_send(channel) && (space.is_dms() || space.permissions(channel) & permissions::SEND_POLLS != 0)
    }

    /// The + at the start of the message box: a menu to upload a file or
    /// create a poll, or the file picker at once where polls are not
    /// allowed.
    pub(in super::super) fn render_plus_button(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let channel = self.selected_channel?;
        let (files, poll) = (self.can_attach(channel), self.can_poll(channel));
        if !poll {
            return self.render_attach_button(cx);
        }
        Some(
            div()
                .id("plus")
                .flex_none()
                // Centered on a one-row message box.
                .mt(zoomed(7.))
                .size(zoomed(32.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(theme::radius::control())
                .cursor_pointer()
                .hover(|s| s.bg(theme::hover()))
                .child(icons::icon(icons::ATTACH, 22., theme::interactive()))
                .on_click(cx.listener(move |this, _, window, cx| {
                    if files {
                        this.plus_menu = !this.plus_menu;
                        cx.notify();
                    } else {
                        this.open_create_poll(window, cx);
                    }
                }))
                .into_any_element(),
        )
    }

    /// The + menu, over the message box's left corner.
    pub(in super::super) fn render_plus_menu(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.plus_menu {
            return None;
        }
        let item = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .px_2()
                .py(zoomed(6.))
                .rounded(theme::radius::tag())
                .cursor_pointer()
                .text_size(zoomed(14.))
                .font_weight(FontWeight::MEDIUM)
                .hover(|s| s.bg(theme::hover()))
                .child(label)
        };
        Some(
            div()
                .absolute()
                .bottom(zoomed(48.))
                .left_0()
                .w(zoomed(200.))
                .p_1()
                .flex()
                .flex_col()
                .rounded(theme::radius::control())
                .border_1()
                .border_color(theme::border())
                .bg(rgb(theme::surface_high()))
                // A press anywhere else closes it, as menus do.
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    this.plus_menu = false;
                    cx.notify();
                }))
                .child(item("plus-upload", t!("upload.attach")).on_click(cx.listener(|this, _, _, cx| {
                    this.plus_menu = false;
                    this.pick_files(cx);
                })))
                .child(item("plus-poll", t!("poll.create")).on_click(cx.listener(|this, _, window, cx| {
                    this.plus_menu = false;
                    this.open_create_poll(window, cx);
                })))
                .into_any_element(),
        )
    }

    fn open_create_poll(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(channel) = self.selected_channel else { return };
        let question = cx.new(|cx| TextInput::new(t!("poll.question_placeholder"), cx).keep_on_submit());
        let answers = (0..2).map(|_| answer_input(cx)).collect();
        window.focus(&question.focus_handle(cx));
        self.overlay = Some(Overlay::CreatePoll(NewPoll {
            channel,
            question,
            answers,
            hours: DEFAULT_HOURS,
            multiple: false,
            error: None,
        }));
        cx.notify();
    }

    pub(in super::super) fn render_poll_modal(&self, modal: &NewPoll, cx: &mut Context<Self>) -> AnyElement {
        let busy = self.busy.contains(&Action::CreatePoll);
        let chip = |id: (&'static str, usize), label: String, on: bool| {
            div()
                .id(id)
                .px_2()
                .py(zoomed(2.))
                .rounded_full()
                .border_1()
                .border_color(if on { rgb(theme::brand()) } else { theme::border() })
                .bg(if on { theme::active() } else { rgb(theme::base_lower()) })
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(if on { theme::text_strong() } else { theme::text_default() }))
                .cursor_pointer()
                .hover(|s| s.bg(theme::hover()))
                .child(label)
        };
        let durations = DURATIONS.iter().enumerate().map(|(ix, &hours)| {
            chip(("poll-duration", ix), time::duration(u64::from(hours) * 3600), modal.hours == hours).on_click(
                cx.listener(move |this, _, _, cx| {
                    if let Some(Overlay::CreatePoll(modal)) = &mut this.overlay {
                        modal.hours = hours;
                        cx.notify();
                    }
                }),
            )
        });
        let add = (modal.answers.len() < MAX_ANSWERS).then(|| {
            div()
                .id("poll-add-answer")
                .text_size(zoomed(14.))
                .text_color(rgb(theme::link()))
                .cursor_pointer()
                .child(t!("poll.add_answer"))
                .on_click(cx.listener(|this, _, window, cx| {
                    let input = answer_input(cx);
                    window.focus(&input.focus_handle(cx));
                    if let Some(Overlay::CreatePoll(modal)) = &mut this.overlay {
                        modal.answers.push(input);
                        cx.notify();
                    }
                }))
        });
        let multiple = modal.multiple;
        let body = div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(widgets::label(t!("poll.question")))
                    .child(modal.question.clone()),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(widgets::label(t!("poll.answers")))
                    .children(modal.answers.iter().cloned())
                    .children(add),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(widgets::label(t!("poll.duration")))
                    .child(div().flex().flex_wrap().gap_1().children(durations)),
            )
            .child(
                div()
                    .id("poll-multiple")
                    .flex()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .child(
                        div()
                            .size(zoomed(18.))
                            .rounded(theme::radius::tag())
                            .border_2()
                            .border_color(rgb(if multiple { theme::brand() } else { theme::interactive_muted() }))
                            .when(multiple, |b| b.bg(rgb(theme::brand()))),
                    )
                    .child(div().text_size(zoomed(14.)).child(t!("poll.multiple")))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(Overlay::CreatePoll(modal)) = &mut this.overlay {
                            modal.multiple = !modal.multiple;
                            cx.notify();
                        }
                    })),
            );
        let submit = widgets::button("poll-post", t!("poll.post"), widgets::ButtonKind::Brand)
            .when(busy, |button| button.opacity(0.5))
            .on_click(cx.listener(|this, _, _, cx| this.post_poll(cx)));
        self.form_modal("poll-modal", t!("poll.create"), body, modal.error.clone(), submit, cx)
    }

    fn post_poll(&mut self, cx: &mut Context<Self>) {
        let Some(Overlay::CreatePoll(modal)) = &mut self.overlay else { return };
        let question: String = modal.question.read(cx).text().trim().chars().take(MAX_QUESTION).collect();
        let answers: Vec<String> = modal
            .answers
            .iter()
            .map(|input| input.read(cx).text().trim().chars().take(MAX_ANSWER).collect::<String>())
            .filter(|answer| !answer.is_empty())
            .collect();
        let error = if question.is_empty() {
            Some(t!("poll.question_required"))
        } else if answers.is_empty() {
            Some(t!("poll.answer_required"))
        } else {
            None
        };
        modal.error = error.map(Into::into);
        if error.is_some() {
            cx.notify();
            return;
        }
        let command = Command::SendPoll {
            channel: modal.channel,
            nonce: rest::nonce(),
            poll: PollToSend {
                question,
                answers,
                multiple: modal.multiple,
                hours: modal.hours,
            },
        };
        self.run(Action::CreatePoll, command, cx);
    }
}

fn answer_input(cx: &mut Context<Emerald>) -> Entity<TextInput> {
    cx.new(|cx| TextInput::new(t!("poll.answer_placeholder"), cx).keep_on_submit())
}
