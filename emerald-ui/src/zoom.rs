//! The interface zoom. Lengths in pixels go through `zoomed`, which multiplies
//! by the chosen percent; rem based lengths (spacing, the text size classes)
//! follow the rem size that `apply` sets on the window.

use std::sync::atomic::{AtomicU32, Ordering};

use gpui::{Pixels, Window, px};

/// Zoom levels, in percent, that the settings offer; smallest first.
pub const LEVELS: [u32; 5] = [90, 100, 110, 125, 150];
/// Keeps a hand-edited settings file from making the interface unusable.
const RANGE: (u32, u32) = (50, 200);
/// One rem at 100%: gpui's default, which the text size classes are based on.
const BASE_REM: f32 = 16.;

static PERCENT: AtomicU32 = AtomicU32::new(100);

/// Chooses the zoom for the next draws, kept within `RANGE`.
pub fn set(percent: u32) {
    PERCENT.store(percent.clamp(RANGE.0, RANGE.1), Ordering::Relaxed);
}

fn factor() -> f32 {
    PERCENT.load(Ordering::Relaxed) as f32 / 100.
}

/// A length in pixels at the chosen zoom.
pub fn zoomed(value: f32) -> Pixels {
    px(value * factor())
}

/// Sets the rem size of `window` to the chosen zoom.
pub fn apply(window: &mut Window) {
    window.set_rem_size(px(BASE_REM * factor()));
}

/// The level one step up or down from `percent`, stopping at the ends.
pub fn step(percent: u32, up: bool) -> u32 {
    if up {
        LEVELS.into_iter().find(|&level| level > percent).unwrap_or(LEVELS[LEVELS.len() - 1])
    } else {
        LEVELS.into_iter().rev().find(|&level| level < percent).unwrap_or(LEVELS[0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_through_levels_and_stops_at_the_ends() {
        assert_eq!(step(100, true), 110);
        assert_eq!(step(100, false), 90);
        // A level picked by hand, between two steps, lands on the next one.
        assert_eq!(step(105, true), 110);
        assert_eq!(step(105, false), 100);
        assert_eq!(step(150, true), 150);
        assert_eq!(step(90, false), 90);
    }

    #[test]
    fn zoom_keeps_within_range() {
        set(1000);
        assert_eq!(PERCENT.load(Ordering::Relaxed), RANGE.1);
        set(1);
        assert_eq!(PERCENT.load(Ordering::Relaxed), RANGE.0);
        set(100);
    }

    #[test]
    fn zoom_keys_parse() {
        for key in ["secondary-=", "secondary--", "secondary-0"] {
            assert!(gpui::Keystroke::parse(key).is_ok(), "{key} should parse");
        }
    }
}
