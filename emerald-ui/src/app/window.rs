//! The window's place and size, kept so the next launch opens where this
//! one closed.

use std::time::Duration;

use gpui::{App, Bounds, Context, Window, WindowAppearance, WindowBounds, point, px, size};

use super::Emerald;
use crate::settings::{Settings, WindowFrame};
use crate::theme;
use crate::zoom;

/// Size of the window when nothing was saved, or the saved place is gone.
const DEFAULT_SIZE: (f32, f32) = (1200., 780.);
/// Moves and resizes come in bursts; the file is written once they settle.
const SAVE_DELAY: Duration = Duration::from_millis(800);

/// Where the window opens: the saved place when a screen still shows part
/// of it, else centered at the default size.
pub fn initial_bounds(settings: &Settings, cx: &App) -> WindowBounds {
    let default = || {
        let (width, height) = DEFAULT_SIZE;
        WindowBounds::Windowed(Bounds::centered(None, size(px(width), px(height)), cx))
    };
    let Some(frame) = settings.window else { return default() };
    let bounds = Bounds::new(point(px(frame.x), px(frame.y)), size(px(frame.width), px(frame.height)));
    let on_screen = cx.displays().iter().any(|display| bounds.intersects(&display.bounds()));
    match (on_screen, frame.maximized) {
        (false, _) => default(),
        (true, true) => WindowBounds::Maximized(bounds),
        (true, false) => WindowBounds::Windowed(bounds),
    }
}

impl Emerald {
    /// Keeps the window's frame in the settings, saving it shortly after
    /// the last move or resize.
    pub(super) fn remember_window(&mut self, window: &Window, cx: &mut Context<Self>) {
        let (bounds, maximized) = match window.window_bounds() {
            WindowBounds::Windowed(bounds) => (bounds, false),
            WindowBounds::Maximized(bounds) => (bounds, true),
            // Full screen is not kept: the next launch opens in a window.
            WindowBounds::Fullscreen(_) => return,
        };
        let frame = WindowFrame {
            x: f32::from(bounds.origin.x),
            y: f32::from(bounds.origin.y),
            width: f32::from(bounds.size.width),
            height: f32::from(bounds.size.height),
            maximized,
        };
        if self.settings.window == Some(frame) {
            return;
        }
        self.settings.window = Some(frame);
        // Replacing the task drops the one still waiting, so only the last counts.
        self.window_save = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SAVE_DELAY).await;
            this.update(cx, |this, cx| this.save_settings(cx)).ok();
        }));
    }
}

impl Emerald {
    /// Draws the chosen style, palette and zoom. The palette follows the system's
    /// when the choice says so.
    pub(super) fn apply_appearance(&self, window: &mut Window) {
        let system_light = matches!(window.appearance(), WindowAppearance::Light | WindowAppearance::VibrantLight);
        theme::set_light(self.settings.theme.is_light(system_light));
        theme::set_style(self.settings.style);
        zoom::set(self.settings.zoom);
        zoom::apply(window);
    }

    /// Zooms one level in or out.
    pub(super) fn zoom_by(&mut self, up: bool, cx: &mut Context<Self>) {
        self.set_zoom(zoom::step(self.settings.zoom, up), cx);
    }

    pub(super) fn zoom_reset(&mut self, cx: &mut Context<Self>) {
        self.set_zoom(100, cx);
    }

    pub(super) fn set_zoom(&mut self, percent: u32, cx: &mut Context<Self>) {
        if self.settings.zoom == percent {
            return;
        }
        self.settings.zoom = percent;
        // A list keeps each row's height until the width changes, so the rows
        // are dropped and measured again at the new zoom, from the same place.
        let top = self.messages.logical_scroll_top();
        let count = self.messages.item_count();
        self.messages.splice(0..count, count);
        self.messages.scroll_to(top);
        self.save_settings(cx);
    }
}

impl Emerald {
    /// Whether animated emoji, avatars and stickers play now: the window has
    /// focus, as in the web client, and reduced motion is off.
    pub(super) fn animates(&self) -> bool {
        self.window_active && !self.settings.reduced_motion
    }
}
