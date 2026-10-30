//! Modify Attachment: alt text and spoiler for one card.

use super::*;
use crate::zoom::zoomed;

impl Emerald {
    /// Opens "Modify Attachment" for the card at `ix`.
    pub(super) fn edit_attachment(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(file) = self.composer.attachments.get(ix) else { return };
        let (description, spoiler) = (file.upload.description.clone(), file.upload.spoiler);
        self.alt_input.update(cx, |input, cx| input.set_text(description, cx));
        self.overlay = Some(Overlay::EditAttachment { ix, spoiler });
        window.focus(&self.alt_input.focus_handle(cx));
        cx.notify();
    }

    /// Keeps what the modal says and closes it.
    pub(in super::super) fn save_attachment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Overlay::EditAttachment { ix, spoiler }) = self.overlay else { return };
        let description: String = self.alt_input.read(cx).text().trim().chars().take(MAX_ALT_TEXT).collect();
        if let Some(file) = self.composer.attachments.get_mut(ix) {
            file.upload.description = description;
            file.upload.spoiler = spoiler;
        }
        self.close_overlay(window, cx);
    }

    /// "Modify Attachment": the alt text and the spoiler switch.
    pub(in super::super) fn render_attachment_modal(&self, ix: usize, spoiler: bool, cx: &mut Context<Self>) -> Option<AnyElement> {
        let file = self.composer.attachments.get(ix)?;
        let preview = file.preview.as_ref().map(Still::image).map(|image| {
            div()
                .relative()
                .h(zoomed(160.))
                .flex()
                .justify_center()
                .rounded(theme::radius::card())
                .overflow_hidden()
                .child(img(image).h_full())
        });
        let modal = div()
            .id("attachment-modal")
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
                    .children(preview)
                    .child(
                        div()
                            .text_size(zoomed(20.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::text_strong()))
                            .truncate()
                            .child(file.upload.name.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(theme::text_default()))
                                    .child(t!("upload.description")),
                            )
                            .child(self.alt_input.clone())
                            .child(div().text_xs().text_color(theme::muted()).child(t!("upload.description_hint"))),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(t!("upload.mark_spoiler"))
                            .child(widgets::toggle("attachment-spoiler-toggle", spoiler).on_click(cx.listener(
                                |this, _, _, cx| {
                                    if let Some(Overlay::EditAttachment { spoiler, .. }) = &mut this.overlay {
                                        *spoiler = !*spoiler;
                                    }
                                    cx.notify();
                                },
                            ))),
                    ),
            )
            .child(
                div()
                    .p_4()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .bg(rgb(theme::base_low()))
                    .child(
                        widgets::button("attachment-cancel", t!("common.cancel"), widgets::ButtonKind::Secondary)
                            .on_click(cx.listener(|this, _, window, cx| this.close_overlay(window, cx))),
                    )
                    .child(
                        widgets::button("attachment-save", t!("upload.save"), widgets::ButtonKind::Brand)
                            .on_click(cx.listener(|this, _, window, cx| this.save_attachment(window, cx))),
                    ),
            );
        Some(
            div()
                .id("attachment-scrim")
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
                .child(modal)
                .into_any_element(),
        )
    }
}
