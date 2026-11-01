//! A short confirmation at the bottom of the window ("Bookmark added",
//! "Message forwarded!"), gone after a few seconds, as the web client's
//! toasts are.

use std::time::Duration;

use super::*;
use gpui::{AnyElement, FontWeight, deferred, svg};
use crate::zoom::zoomed;

/// How long a toast stays.
const SHOWN: Duration = Duration::from_secs(3);

pub(super) struct Toast {
    text: SharedString,
    icon: Option<&'static str>,
    /// Tells a toast's timer apart from a newer toast's.
    serial: u64,
}

impl Emerald {
    /// Shows `text` with an optional icon, replacing any toast on screen.
    pub(super) fn show_toast(&mut self, text: impl Into<SharedString>, icon: Option<&'static str>, cx: &mut Context<Self>) {
        let serial = self.toast.as_ref().map_or(0, |t| t.serial + 1);
        self.toast = Some(Toast {
            text: text.into(),
            icon,
            serial,
        });
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SHOWN).await;
            this.update(cx, |this, cx| {
                if this.toast.as_ref().is_some_and(|t| t.serial == serial) {
                    this.toast = None;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn render_toast(&self) -> Option<AnyElement> {
        let toast = self.toast.as_ref()?;
        Some(
            deferred(
                div()
                    .absolute()
                    .bottom(zoomed(24.))
                    .left_0()
                    .right_0()
                    .flex()
                    .justify_center()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_3()
                            .py_2()
                            .rounded(theme::radius::control())
                            .border_1()
                            .border_color(theme::border_normal())
                            .bg(rgb(theme::surface_high()))
                            .shadow_lg()
                            .text_size(zoomed(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .children(toast.icon.map(|path| {
                                svg().path(path).size(zoomed(16.)).text_color(theme::text())
                            }))
                            .child(toast.text.clone()),
                    ),
            )
            .with_priority(3)
            .into_any_element(),
        )
    }
}
