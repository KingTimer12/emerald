//! Files for the next message: picked with the + button, dropped on the
//! chat or pasted, shown as cards above the message box and sent with it.

use crate::zoom::zoomed;

mod modal;
mod picking;

use picking::*;

use std::path::PathBuf;

use gpui::{
    AnyElement, Context, ExternalPaths, Focusable, FontWeight, Image, ImageFormat, MouseButton, PathPromptOptions,
    SharedString, Window, div, img, prelude::*, rgb,
};

use super::super::icons::{self, icon};
use super::super::theme;
use super::{Emerald, Overlay, Still, widgets};
use emerald_discord::Upload;

/// Files one message may carry, as in Discord.
pub const MAX_FILES: usize = 10;
const MB: u64 = 1024 * 1024;
/// Discord's limit on an attachment's alt text.
const MAX_ALT_TEXT: usize = 1024;
const CARD: f32 = 160.;

/// A file waiting in the composer.
pub(super) struct Attachment {
    pub upload: Upload,
    /// Images show themselves on the card; a photo's pixels are large, so
    /// they leave memory with the card.
    preview: Option<Still>,
}

impl Attachment {
    pub fn new(upload: Upload) -> Self {
        let preview = preview_format(&upload.name).map(|format| Still::new(format, upload.bytes.to_vec()));
        Self { upload, preview }
    }
}

/// Why a file was left out.
enum Refused {
    TooLarge(String),
    Unreadable(String),
}

impl Emerald {

    /// The cards above the message box, or in the "New Post" form. There
    /// `editable` is off: "Modify Attachment" would replace the form.
    pub(super) fn render_attachments(&self, editable: bool, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.composer.attachments.is_empty() {
            return None;
        }
        let cards = self.composer.attachments.iter().enumerate().map(|(ix, file)| {
            let shown = match &file.preview {
                Some(image) => img(image.image()).size_full().rounded_md().into_any_element(),
                None => div().text_size(zoomed(48.)).child("📄").into_any_element(),
            };
            let spoiler = file.upload.spoiler;
            let tools = div()
                .absolute()
                .top(zoomed(-8.))
                .right(zoomed(-8.))
                .flex()
                .rounded_md()
                .overflow_hidden()
                .border_1()
                .border_color(theme::border())
                .bg(rgb(theme::surface_high()))
                .child(
                    tool(("attachment-spoiler", ix), icons::EYE, t!("upload.spoiler"), spoiler).on_click(
                        cx.listener(move |this, _, _, cx| {
                            if let Some(file) = this.composer.attachments.get_mut(ix) {
                                file.upload.spoiler = !file.upload.spoiler;
                            }
                            cx.notify();
                        }),
                    ),
                )
                .when(editable, |tools| {
                    tools.child(
                        tool(("attachment-edit", ix), icons::EDIT, t!("upload.edit"), false)
                            .on_click(cx.listener(move |this, _, window, cx| this.edit_attachment(ix, window, cx))),
                    )
                })
                .child(
                    tool(("attachment-remove", ix), icons::TRASH, t!("upload.remove"), false).on_click(
                        cx.listener(move |this, _, _, cx| {
                            if ix < this.composer.attachments.len() {
                                this.composer.attachments.remove(ix);
                            }
                            cx.notify();
                        }),
                    ),
                );
            div()
                .relative()
                .w(zoomed(CARD))
                .flex_none()
                .flex()
                .flex_col()
                .gap_2()
                .p_2()
                .rounded_md()
                .bg(rgb(theme::base_low()))
                .child(
                    div()
                        .relative()
                        .h(zoomed(CARD - 48.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_md()
                        .overflow_hidden()
                        .child(shown)
                        // Covered as readers will first see it.
                        .when(spoiler, |frame| {
                            frame.child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(rgb(theme::base_lowest()))
                                    .child(super::chat::spoiler_pill()),
                            )
                        }),
                )
                .child(div().text_size(zoomed(14.)).truncate().child(file.upload.name.clone()))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::muted())
                        .truncate()
                        .child(if file.upload.description.is_empty() {
                            format_size(file.upload.bytes.len() as u64)
                        } else {
                            format!("{} · {}", t!("chat.alt"), file.upload.description).into()
                        }),
                )
                .child(tools)
        });
        Some(
            div()
                .id("attachments")
                .flex()
                .gap_3()
                .p_3()
                .pt_4()
                .overflow_x_scroll()
                .rounded_t_lg()
                .bg(rgb(theme::base_lower()))
                .children(cards)
                .into_any_element(),
        )
    }

    /// The + at the start of the message box.
    pub(super) fn render_attach_button(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let channel = self.selected_channel?;
        if !self.can_attach(channel) {
            return None;
        }
        Some(
            div()
                .id("attach")
                .flex_none()
                // Centered on a one-row message box.
                .mt(zoomed(7.))
                .size(zoomed(32.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_md()
                .cursor_pointer()
                .hover(|s| s.bg(theme::hover()))
                .tooltip(widgets::tooltip(t!("upload.attach")))
                .child(icon(icons::ATTACH, 22., theme::interactive()))
                .on_click(cx.listener(|this, _, _, cx| this.pick_files(cx)))
                .into_any_element(),
        )
    }
}

/// A button on a card's toolbar, lit while `on`.
fn tool(id: (&'static str, usize), path: &'static str, label: &'static str, on: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .size(zoomed(28.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|s| s.bg(theme::hover()))
        .tooltip(widgets::tooltip(label))
        .child(icon(path, 18., if on { theme::brand() } else { theme::interactive() }))
}

/// `1.5 MB`, in the units Discord uses for its limits.
pub(super) fn format_size(bytes: u64) -> SharedString {
    const KB: f64 = 1024.;
    let bytes = bytes as f64;
    if bytes < KB * KB {
        format!("{:.0} KB", (bytes / KB).ceil()).into()
    } else {
        format!("{:.1} MB", bytes / KB / KB).replace(".0 ", " ").into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_like_discord() {
        assert_eq!(format_size(10 * 1024 * 1024), "10 MB");
        assert_eq!(format_size(1536 * 1024), "1.5 MB");
        assert_eq!(format_size(100), "1 KB");
    }

    #[test]
    fn previews_only_images() {
        assert_eq!(preview_format("a.PNG"), Some(ImageFormat::Png));
        assert_eq!(preview_format("notes.txt"), None);
        assert_eq!(preview_format("png"), None);
    }
}
