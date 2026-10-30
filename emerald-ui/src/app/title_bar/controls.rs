//! Minimize, maximize and close, drawn on Windows where the system's own
//! buttons are gone with its title bar. Each is a window-control area, so
//! Windows still runs them itself, snap layouts on maximize included.

use gpui::{AnyElement, Window, WindowControlArea, div, prelude::*, rgb};

use crate::icons;
use crate::theme;
use crate::zoom::zoomed;

/// Windows' own button width beside a 32px bar.
const WIDTH: f32 = 46.;
/// The close button's hover red, as Windows paints it.
const CLOSE_HOVER: u32 = 0xe81123;

pub(super) fn window_controls(window: &Window) -> AnyElement {
    let maximize = if window.is_maximized() { icons::WINDOW_RESTORE } else { icons::WINDOW_MAXIMIZE };
    div()
        .flex()
        .h_full()
        .child(control("window-minimize", icons::WINDOW_MINIMIZE, WindowControlArea::Min, false))
        .child(control("window-maximize", maximize, WindowControlArea::Max, false))
        .child(control("window-close", icons::WINDOW_CLOSE, WindowControlArea::Close, true))
        .into_any_element()
}

fn control(id: &'static str, glyph: &'static str, area: WindowControlArea, close: bool) -> impl IntoElement {
    div()
        .id(id)
        .group(id)
        .w(zoomed(WIDTH))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .window_control_area(area)
        // Stops the hit test here, or the bar's drag area under the button
        // would answer first and Windows would move the window instead.
        .occlude()
        .hover(move |s| if close { s.bg(rgb(CLOSE_HOVER)) } else { s.bg(theme::hover()) })
        .child(
            icons::icon(glyph, 24., theme::interactive())
                .group_hover(id, move |s| if close { s.text_color(rgb(theme::on_color())) } else { s }),
        )
}
