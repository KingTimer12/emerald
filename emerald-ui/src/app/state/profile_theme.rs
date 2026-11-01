//! A profile's theme colors as the web client turns them into a card:
//! light or dark, the overlay, readable text and base colors, and a button
//! color that keeps white text legible. Also the color a styled display
//! name is drawn in.

use emerald_discord::model::NameStyle;

/// What a profile card with theme colors is drawn with, all RGB.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProfileTheme {
    /// Dark card, light text.
    pub dark: bool,
    pub primary: u32,
    pub accent: u32,
    /// Laid over the gradient: black on a dark card, white on a light one,
    /// with its opacity.
    pub overlay: (u32, f32),
    pub text: u32,
    /// Behind boxes and sections on the card.
    pub base: u32,
    /// Filled buttons, with white text.
    pub button: u32,
}

/// What a card's name is drawn over: its base color when themed, else the
/// usual surface.
pub fn card_background(theme: Option<&ProfileTheme>) -> u32 {
    theme.map_or(crate::theme::surface_high(), |t| t.base)
}

/// The theme of a profile whose colors are `primary` and `accent`.
pub fn profile_theme(primary: u32, accent: u32) -> ProfileTheme {
    let dark = darkness(primary) > 0.5;
    let (lighter, darker) = if luminance(primary) >= luminance(accent) {
        (primary, accent)
    } else {
        (accent, primary)
    };
    let (lh, ls, _) = hsl(lighter);
    let (dh, ds, _) = hsl(darker);
    // A grey has no hue to keep; the web client then uses the lighter one's.
    let (dh, ds) = if ds == 0. { (lh, ls) } else { (dh, ds) };
    let saturated = |s: f32| if s == 0. { 0. } else { 1. };
    let blue = |h: f32| (200. ..300.).contains(&h);
    let (text, base) = if dark {
        let text = from_hsl(lh, saturated(ls), if blue(lh) { 0.8 } else { 0.95 });
        (text, from_hsl(dh, saturated(ds), 0.05))
    } else {
        let text = from_hsl(dh, saturated(ds), if blue(dh) { 0.25 } else { 0.15 });
        (text, from_hsl(lh, saturated(ls), 0.9))
    };
    ProfileTheme {
        dark,
        primary,
        accent,
        overlay: (if dark { 0x000000 } else { 0xffffff }, 0.6),
        text,
        base,
        button: button_color(primary),
    }
}

/// The color a display name with `style` is drawn in on `background`: its
/// first color, pushed lighter or darker until it reaches the contrast the
/// web client asks of the effect. gpui draws one color per text run, so
/// gradients, glows and the like show as that color. `None` without colors.
pub fn name_color(style: &NameStyle, background: u32) -> Option<u32> {
    let first = *style.colors.first()?;
    // Gradient, glow and prism ask less of each of their colors.
    let target = if matches!(style.effect_id, 2 | 6 | 7) { 2.5 } else { 3. };
    let (h, s, mut l) = hsl(first);
    let step = if luminance(background) < 0.18 { 0.02 } else { -0.02 };
    let mut color = first;
    for _ in 0..50 {
        if contrast(color, background) >= target {
            break;
        }
        l = (l + step).clamp(0., 1.);
        color = from_hsl(h, s, l);
    }
    Some(color)
}

/// `primary` stepped darker or lighter until white on it has a contrast
/// between 7 and 10.
fn button_color(primary: u32) -> u32 {
    let (h, s, mut l) = hsl(primary);
    let mut color = primary;
    for _ in 0..100 {
        let ratio = contrast(color, 0xffffff);
        if (7.0..=10.0).contains(&ratio) {
            break;
        }
        l = (l + if ratio < 7. { -0.01 } else { 0.01 }).clamp(0., 1.);
        color = from_hsl(h, s, l);
    }
    color
}

/// 1 for black, 0 for white, by perceived brightness.
fn darkness(rgb: u32) -> f32 {
    let (r, g, b) = channels(rgb);
    1. - (0.299 * r + 0.587 * g + 0.114 * b) / 255.
}

fn channels(rgb: u32) -> (f32, f32, f32) {
    ((rgb >> 16 & 0xff) as f32, (rgb >> 8 & 0xff) as f32, (rgb & 0xff) as f32)
}

/// WCAG relative luminance.
fn luminance(rgb: u32) -> f32 {
    let linear = |c: f32| {
        let c = c / 255.;
        if c <= 0.039_28 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    let (r, g, b) = channels(rgb);
    0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
}

/// WCAG contrast ratio of two colors, 1 to 21.
pub fn contrast(a: u32, b: u32) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// Hue in degrees, saturation and lightness 0 to 1.
fn hsl(rgb: u32) -> (f32, f32, f32) {
    let (r, g, b) = channels(rgb);
    let (r, g, b) = (r / 255., g / 255., b / 255.);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.;
    let d = max - min;
    if d == 0. {
        return (0., 0., l);
    }
    let s = d / (1. - (2. * l - 1.).abs());
    let h = if max == r {
        60. * ((g - b) / d).rem_euclid(6.)
    } else if max == g {
        60. * ((b - r) / d + 2.)
    } else {
        60. * ((r - g) / d + 4.)
    };
    (h, s, l)
}

fn from_hsl(h: f32, s: f32, l: f32) -> u32 {
    let c = (1. - (2. * l - 1.).abs()) * s;
    let x = c * (1. - ((h / 60.).rem_euclid(2.) - 1.).abs());
    let m = l - c / 2.;
    let (r, g, b) = match h as u32 {
        0..=59 => (c, x, 0.),
        60..=119 => (x, c, 0.),
        120..=179 => (0., c, x),
        180..=239 => (0., x, c),
        240..=299 => (x, 0., c),
        _ => (c, 0., x),
    };
    let byte = |v: f32| ((v + m) * 255.).round().clamp(0., 255.) as u32;
    byte(r) << 16 | byte(g) << 8 | byte(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn near_black_is_dark_and_near_white_is_light() {
        assert!(profile_theme(0x202020, 0x5865f2).dark);
        assert!(!profile_theme(0xf0f0f0, 0x5865f2).dark);
    }

    #[test]
    fn text_stays_readable_on_its_base() {
        for (primary, accent) in [(0x202020, 0x5865f2), (0xf0f0f0, 0xffd1dc), (0x000000, 0x000000), (0xffffff, 0xffffff)] {
            let theme = profile_theme(primary, accent);
            assert!(contrast(theme.text, theme.base) >= 4.5, "{primary:06x}/{accent:06x}");
        }
    }

    #[test]
    fn buttons_keep_white_text_legible() {
        for primary in [0x202020, 0xf0f0f0, 0x5865f2, 0xff0000, 0x00ff00] {
            let ratio = contrast(profile_theme(primary, 0).button, 0xffffff);
            assert!((7.0..=10.5).contains(&ratio), "{primary:06x}: {ratio}");
        }
    }

    #[test]
    fn dark_names_lighten_on_dark_backgrounds() {
        let style = NameStyle { font_id: 4, effect_id: 1, colors: vec![0x101020] };
        let color = name_color(&style, 0x1a1a1e).unwrap();
        assert!(contrast(color, 0x1a1a1e) >= 3.);
        let gradient = NameStyle { effect_id: 2, ..style.clone() };
        assert!(contrast(name_color(&gradient, 0xffffff).unwrap(), 0xffffff) >= 2.5);
        assert_eq!(name_color(&NameStyle::default(), 0), None);
    }

    #[test]
    fn names_on_light_cards_darken() {
        let theme = profile_theme(0xf5f5f5, 0xffffff);
        let background = card_background(Some(&theme));
        let style = NameStyle { font_id: 0, effect_id: 1, colors: vec![0xfafafa] };
        assert!(contrast(name_color(&style, background).unwrap(), background) >= 3.);
        assert_eq!(card_background(None), crate::theme::surface_high());
    }

    #[test]
    fn colors_round_trip_through_hsl() {
        for rgb in [0x5865f2, 0x202020, 0xff8800] {
            let (h, s, l) = hsl(rgb);
            let back = from_hsl(h, s, l);
            assert!(channels(back).0.sub_abs(channels(rgb).0) <= 1.);
        }
    }

    trait SubAbs {
        fn sub_abs(self, other: f32) -> f32;
    }

    impl SubAbs for f32 {
        fn sub_abs(self, other: f32) -> f32 {
            (self - other).abs()
        }
    }
}
