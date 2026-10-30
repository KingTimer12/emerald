//! The look of the window: the visual style (Discord's 2026 look for now),
//! its dark or light palette, and the colors and fonts each style uses.
//!
//! Every value is a function that reads the switches in use, so one switch
//! (`set_light`, `set_style`) restyles the whole window on its next draw.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

mod colors;
pub mod fonts;
pub mod radius;

pub use colors::*;
pub use fonts::MONO;

use crate::settings::Style;

static LIGHT: AtomicBool = AtomicBool::new(false);
/// The style in use, as its index in `Style::ALL`.
static STYLE: AtomicU8 = AtomicU8::new(0);

/// Switches every value to `style`'s.
pub fn set_style(style: Style) {
    let index = Style::ALL.iter().position(|known| *known == style).unwrap_or(0);
    STYLE.store(index as u8, Ordering::Relaxed);
}

/// The style in use.
pub fn style() -> Style {
    Style::ALL.get(STYLE.load(Ordering::Relaxed) as usize).copied().unwrap_or_default()
}

/// Switches every color to the light palette, or back to the dark one.
pub fn set_light(light: bool) {
    LIGHT.store(light, Ordering::Relaxed);
}

/// Whether the light palette is in use.
pub fn is_light() -> bool {
    LIGHT.load(Ordering::Relaxed)
}

/// Tests that switch the window-wide palette, style or zoom hold this, so
/// tests running in parallel never see each other's switches.
#[cfg(test)]
pub(crate) fn lock_switches() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn pick(dark: u32, light: u32) -> u32 {
    if is_light() { light } else { dark }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_switch_is_read_back() {
        let _switches = lock_switches();
        set_style(Style::Y2026);
        assert_eq!(style(), Style::Y2026);
    }

    #[test]
    fn light_palette_only_while_switched() {
        let _switches = lock_switches();
        set_light(false);
        assert_eq!(base_lowest(), 0x2c2d32);
        set_light(true);
        assert_eq!(base_lowest(), 0xf3f3f4);
        assert_eq!(brand(), 0x5865f2);
        set_light(false);
    }
}
