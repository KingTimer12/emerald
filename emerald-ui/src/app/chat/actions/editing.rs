//! Replying and editing in place: starting, saving and cancelling, and their bars.

use super::*;
use crate::zoom::zoomed;

impl Emerald {
    pub(in super::super::super) fn start_reply(&mut self, channel: Snowflake, id: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
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

    pub(in super::super::super) fn start_edit(&mut self, channel: Snowflake, id: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
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
    pub(in super::super::super) fn edit_last(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
    pub(super) fn stop_editing(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(edit) = self.edit.take() else { return false };
        self.redraw_message(edit.channel, edit.message);
        cx.notify();
        true
    }

    pub(super) fn cancel_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.stop_editing(cx) {
            window.focus(&self.composer.input.focus_handle(cx));
        }
    }

    /// Enter in the edit box. The new text shows at once; the gateway echo
    /// confirms it.
    pub(in super::super::super) fn save_edit(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
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
            poll: None,
        };
        let view = shown(&self.messages, self.selected_channel, channel);
        self.history.edit(update, view);
        self.net.post(Command::EditMessage {
            channel,
            message,
            content: text,
        });
    }
}

impl Emerald {
    /// The edit box in place of a message's text.
    pub(in super::super) fn render_edit_box(&self) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .py_1()
            .child(self.edit_input.clone())
            .child(div().text_xs().text_color(theme::muted()).child(t!("message.edit_hint")))
    }

    /// "Replying to …" above the message box, with the mention switch.
    pub(in super::super) fn render_reply_bar(&self, cx: &mut Context<Self>) -> Option<Div> {
        let reply = self.composer.reply.as_ref().filter(|r| Some(r.channel) == self.selected_channel)?;
        let (text, names) = t_spans!("message.replying_to", name = reply.name);
        let mention = reply.mention;
        let mine = self.people.is_me(reply.preview.author_id);
        Some(
            div()
                .flex()
                .items_center()
                .gap_2()
                .h(zoomed(34.))
                .px_4()
                .rounded_t_lg()
                .bg(rgb(theme::base_low()))
                .text_size(zoomed(14.))
                .text_color(rgb(theme::text_muted()))
                .child(div().flex_1().min_w(zoomed(0.)).truncate().child(gpui::StyledText::new(text).with_highlights(
                    names.into_iter().map(|range| {
                        (
                            range,
                            gpui::HighlightStyle {
                                font_weight: Some(FontWeight::SEMIBOLD),
                                color: Some(rgb(theme::text_default()).into()),
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
                            .text_size(zoomed(12.))
                            .text_color(rgb(if mention { theme::link() } else { theme::text_muted() }))
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
                .child(div().w(zoomed(1.)).h(zoomed(16.)).bg(theme::border()))
                .child(
                    div()
                        .id("reply-cancel")
                        .flex_none()
                        .size(zoomed(18.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .cursor_pointer()
                        .bg(rgb(theme::interactive()))
                        .hover(|s| s.bg(rgb(theme::text_strong())))
                        .text_size(zoomed(10.))
                        .text_color(rgb(theme::base_low()))
                        .child("✕")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.composer.reply = None;
                            cx.notify();
                        })),
                ),
        )
    }
}
