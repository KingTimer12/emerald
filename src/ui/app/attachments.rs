//! Files for the next message: picked with the + button, dropped on the
//! chat or pasted, shown as cards above the message box and sent with it.

use std::path::PathBuf;
use std::sync::Arc;

use gpui::{
    AnyElement, Context, ExternalPaths, Focusable, FontWeight, Image, ImageFormat, MouseButton, PathPromptOptions,
    SharedString, Window, div, img, prelude::*, px, rgb,
};

use super::super::icons::{self, icon};
use super::super::theme;
use super::{Emerald, Overlay, widgets};
use crate::discord::Upload;

/// Files one message may carry, as in Discord.
pub const MAX_FILES: usize = 10;
const MB: u64 = 1024 * 1024;
/// Discord's limit on an attachment's alt text.
const MAX_ALT_TEXT: usize = 1024;
const CARD: f32 = 160.;

/// A file waiting in the composer.
pub(super) struct Attachment {
    pub upload: Upload,
    /// Images show themselves on the card.
    preview: Option<Arc<Image>>,
}

impl Attachment {
    pub fn new(upload: Upload) -> Self {
        let preview = preview_format(&upload.name)
            .map(|format| Arc::new(Image::from_bytes(format, upload.bytes.to_vec())));
        Self { upload, preview }
    }
}

/// Why a file was left out.
enum Refused {
    TooLarge(String),
    Unreadable(String),
}

impl Emerald {
    /// The + button: the system file picker.
    pub(super) fn pick_files(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else { return };
            this.update(cx, |this, cx| this.attach_paths(paths, cx)).ok();
        })
        .detach();
    }

    /// Files dropped on the chat.
    pub(super) fn on_drop_files(&mut self, paths: &ExternalPaths, cx: &mut Context<Self>) {
        self.attach_paths(paths.paths().to_vec(), cx);
    }

    /// Reads `paths` off the UI thread, refusing what is over the limit
    /// before reading it.
    fn attach_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if !self.may_attach() {
            return;
        }
        let limit = self.upload_limit();
        let read = cx.background_spawn(async move {
            paths
                .into_iter()
                .filter(|path| path.is_file())
                .map(|path| read_file(path, limit))
                .collect::<Vec<_>>()
        });
        cx.spawn(async move |this, cx| {
            let files = read.await;
            this.update(cx, |this, cx| {
                for file in files {
                    match file {
                        Ok(upload) => this.attach(upload),
                        Err(Refused::TooLarge(name)) => {
                            let max = format_size(this.upload_limit());
                            this.attach_error(t!("upload.too_large", name = name, max = max));
                        }
                        Err(Refused::Unreadable(name)) => {
                            this.attach_error(t!("upload.unreadable", name = name));
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// An image pasted into the message box.
    pub(super) fn attach_pasted(&mut self, image: &Image, cx: &mut Context<Self>) {
        if !self.may_attach() {
            return;
        }
        if image.bytes.len() as u64 > self.upload_limit() {
            let max = format_size(self.upload_limit());
            self.attach_error(t!("upload.too_large", name = "image", max = max));
        } else {
            self.attach(Upload {
                name: format!("image.{}", extension(image.format)),
                bytes: image.bytes.clone().into(),
                spoiler: false,
                description: String::new(),
            });
        }
        cx.notify();
    }

    fn attach(&mut self, upload: Upload) {
        if self.composer.attachments.len() >= MAX_FILES {
            self.attach_error(t!("upload.too_many", max = MAX_FILES));
            return;
        }
        self.composer.error = None;
        self.composer.attachments.push(Attachment::new(upload));
    }

    /// Why a file was left out, shown under the form it was meant for: the
    /// "New Post" form while it is open, else the message box.
    fn attach_error(&mut self, error: impl Into<SharedString>) {
        match &mut self.overlay {
            Some(Overlay::CreatePost(modal)) => modal.error = Some(error.into()),
            _ => self.composer.error = Some(error.into()),
        }
    }

    /// The largest file the open channel takes, as the web client works it
    /// out: the better of the guild's boosts and the user's Nitro, and never
    /// under 20 MB. Discord checks again and refuses anything larger.
    fn upload_limit(&self) -> u64 {
        let nitro = match self.people.me().and_then(|me| me.premium_type) {
            Some(2) => 500 * MB,
            Some(1 | 3) => 50 * MB,
            _ => 0,
        };
        let guild = self.spaces.current().info().and_then(|g| g.upload_limit()).unwrap_or(10 * MB);
        guild.max(nitro).max(20 * MB)
    }

    /// Whether files can go to the open channel at all.
    fn may_attach(&self) -> bool {
        self.selected_channel.is_some_and(|channel| self.can_attach(channel))
    }

    /// The cards above the message box, or in the "New Post" form. There
    /// `editable` is off: "Modify Attachment" would replace the form.
    pub(super) fn render_attachments(&self, editable: bool, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.composer.attachments.is_empty() {
            return None;
        }
        let cards = self.composer.attachments.iter().enumerate().map(|(ix, file)| {
            let shown = match &file.preview {
                Some(image) => img(image.clone()).size_full().rounded_md().into_any_element(),
                None => div().text_size(px(48.)).child("📄").into_any_element(),
            };
            let spoiler = file.upload.spoiler;
            let tools = div()
                .absolute()
                .top(px(-8.))
                .right(px(-8.))
                .flex()
                .rounded_md()
                .overflow_hidden()
                .border_1()
                .border_color(theme::border())
                .bg(rgb(theme::SURFACE_HIGH))
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
                .w(px(CARD))
                .flex_none()
                .flex()
                .flex_col()
                .gap_2()
                .p_2()
                .rounded_md()
                .bg(rgb(theme::BASE_LOW))
                .child(
                    div()
                        .relative()
                        .h(px(CARD - 48.))
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
                                    .bg(rgb(theme::BASE_LOWEST))
                                    .child(super::chat::spoiler_pill()),
                            )
                        }),
                )
                .child(div().text_size(px(14.)).truncate().child(file.upload.name.clone()))
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
                .bg(rgb(theme::BASE_LOWER))
                .children(cards)
                .into_any_element(),
        )
    }

    /// Opens "Modify Attachment" for the card at `ix`.
    fn edit_attachment(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(file) = self.composer.attachments.get(ix) else { return };
        let (description, spoiler) = (file.upload.description.clone(), file.upload.spoiler);
        self.alt_input.update(cx, |input, cx| input.set_text(description, cx));
        self.overlay = Some(Overlay::EditAttachment { ix, spoiler });
        window.focus(&self.alt_input.focus_handle(cx));
        cx.notify();
    }

    /// Keeps what the modal says and closes it.
    pub(super) fn save_attachment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Overlay::EditAttachment { ix, spoiler }) = self.overlay else { return };
        let description: String = self.alt_input.read(cx).text().trim().chars().take(MAX_ALT_TEXT).collect();
        if let Some(file) = self.composer.attachments.get_mut(ix) {
            file.upload.description = description;
            file.upload.spoiler = spoiler;
        }
        self.close_overlay(window, cx);
    }

    /// "Modify Attachment": the alt text and the spoiler switch.
    pub(super) fn render_attachment_modal(&self, ix: usize, spoiler: bool, cx: &mut Context<Self>) -> Option<AnyElement> {
        let file = self.composer.attachments.get(ix)?;
        let preview = file.preview.clone().map(|image| {
            div()
                .relative()
                .h(px(160.))
                .flex()
                .justify_center()
                .rounded_md()
                .overflow_hidden()
                .child(img(image).h_full())
        });
        let modal = div()
            .id("attachment-modal")
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
                    .children(preview)
                    .child(
                        div()
                            .text_size(px(20.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT_STRONG))
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
                                    .text_color(rgb(theme::TEXT_DEFAULT))
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
                    .bg(rgb(theme::BASE_LOW))
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
                .mt(px(7.))
                .size(px(32.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_md()
                .cursor_pointer()
                .hover(|s| s.bg(theme::hover()))
                .tooltip(widgets::tooltip(t!("upload.attach")))
                .child(icon(icons::ATTACH, 22., theme::INTERACTIVE))
                .on_click(cx.listener(|this, _, _, cx| this.pick_files(cx)))
                .into_any_element(),
        )
    }
}

/// A button on a card's toolbar, lit while `on`.
fn tool(id: (&'static str, usize), path: &'static str, label: &'static str, on: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .size(px(28.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|s| s.bg(theme::hover()))
        .tooltip(widgets::tooltip(label))
        .child(icon(path, 18., if on { theme::BRAND } else { theme::INTERACTIVE }))
}

fn read_file(path: PathBuf, limit: u64) -> Result<Upload, Refused> {
    let name = path
        .file_name()
        .map_or_else(|| "file".to_owned(), |n| n.to_string_lossy().into_owned());
    match std::fs::metadata(&path) {
        Ok(meta) if meta.len() > limit => return Err(Refused::TooLarge(name)),
        Ok(_) => {}
        Err(_) => return Err(Refused::Unreadable(name)),
    }
    match std::fs::read(&path) {
        // Re-checked: the file may have grown since.
        Ok(bytes) if bytes.len() as u64 > limit => Err(Refused::TooLarge(name)),
        Ok(bytes) => Ok(Upload {
            name,
            bytes: bytes.into(),
            spoiler: false,
            description: String::new(),
        }),
        Err(_) => Err(Refused::Unreadable(name)),
    }
}

/// The image format a card can preview, from the file name.
fn preview_format(name: &str) -> Option<ImageFormat> {
    let extension = name.rsplit_once('.')?.1.to_ascii_lowercase();
    match extension.as_str() {
        "png" => Some(ImageFormat::Png),
        "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
        "gif" => Some(ImageFormat::Gif),
        "webp" => Some(ImageFormat::Webp),
        _ => None,
    }
}

fn extension(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Jpeg => "jpg",
        ImageFormat::Gif => "gif",
        ImageFormat::Webp => "webp",
        ImageFormat::Svg => "svg",
        ImageFormat::Bmp => "bmp",
        ImageFormat::Tiff => "tiff",
        _ => "png",
    }
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
