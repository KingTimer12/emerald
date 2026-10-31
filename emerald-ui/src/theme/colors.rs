//! Discord's dark and light palettes (2025 visual refresh), resolved from the
//! `.theme-dark` and `.theme-light` design tokens in the web client's CSS on
//! 2026-10-19 and 2026-10-27 with the default saturation factor and no custom
//! theme. Names follow the CSS variables.

use gpui::{Hsla, Rgba, rgb, rgba};

use super::pick;

/// `--background-base-lowest`: server list and channel sidebar.
pub fn base_lowest() -> u32 {
    pick(0x2c2d32, 0xf3f3f4)
}

/// `--background-base-lower`: chat column.
pub fn base_lower() -> u32 {
    pick(0x323339, 0xfbfbfb)
}

/// `--background-base-low`: server icons without an image.
pub fn base_low() -> u32 {
    pick(0x36373e, 0xfbfbfb)
}

/// `--chat-background-default`: message box and the user panel surface.
pub fn surface_high() -> u32 {
    pick(0x393a41, 0xffffff)
}

/// `--background-brand`.
pub fn brand() -> u32 {
    0x5865f2
}

/// `--white`: text and icons on brand, danger and positive fills, in both palettes.
pub fn on_color() -> u32 {
    0xffffff
}

/// `--status-danger`.
pub fn danger() -> u32 {
    pick(0xf23f43, 0xda373c)
}

/// `--status-online`.
pub fn online() -> u32 {
    pick(0x23a55a, 0x24934e)
}

/// `--text-default`.
pub fn text_default() -> u32 {
    pick(0xf3f3f4, 0x2e2e34)
}

/// `--text-strong`, also `--interactive-text-hover` / `-active`.
pub fn text_strong() -> u32 {
    pick(0xffffff, 0x28282d)
}

/// `--text-muted`.
pub fn text_muted() -> u32 {
    pick(0xabacb2, 0x6c6d76)
}

/// `--interactive-text-default`.
pub fn interactive() -> u32 {
    pick(0xc5c6ca, 0x595a63)
}

/// `--channels-default`, also `--channel-icon`.
pub fn channels_default() -> u32 {
    pick(0x999aa1, 0x666770)
}

/// `--interactive-muted`: channels the user cannot read.
pub fn interactive_muted() -> u32 {
    pick(0x4e5058, 0xc4c9ce)
}

/// `--channel-text-area-placeholder`.
pub fn placeholder() -> u32 {
    pick(0x6d6f78, 0x80848e)
}

/// `--text-link`.
pub fn link() -> u32 {
    pick(0x76aff6, 0x006dd4)
}

/// `--text-feedback-positive`.
pub fn feedback_positive() -> u32 {
    pick(0x7ecb94, 0x01713a)
}

/// `--text-feedback-critical`.
pub fn feedback_critical() -> u32 {
    pick(0xffa09b, 0xb92733)
}

/// `--control-primary-background-hover`.
pub fn button_brand_hover() -> u32 {
    0x4452bb
}

/// `--control-connected-background-default` / `-hover`.
pub fn button_positive() -> u32 {
    0x008545
}

pub fn button_positive_hover() -> u32 {
    0x006c37
}

/// `--control-critical-primary-background-default` / `-hover`, also
/// `--background-feedback-notification` (mention badges).
pub fn button_danger() -> u32 {
    0xd22d39
}

pub fn button_danger_hover() -> u32 {
    0xa9232e
}

/// Presence colors; idle and offline are not in the theme tokens, these are
/// Discord's long-standing values.
pub fn idle() -> u32 {
    0xf0b232
}

pub fn offline() -> u32 {
    0x80848e
}

/// `--background-scrim`: black at 72%, or 52% on light.
pub fn scrim() -> Rgba {
    rgba(pick(0x000000b8, 0x00000085))
}

/// `color` (RGB) at `alpha`, for tints such as a folder's color.
pub fn with_alpha(color: u32, alpha: f32) -> Rgba {
    rgba(color << 8 | (alpha.clamp(0., 1.) * 255.) as u32)
}

/// Every hover, selection and border token is `#97979f` at some alpha.
fn overlay(alpha: u32) -> Rgba {
    rgba(0x97979f00 | alpha)
}

/// `--interactive-background-hover`, also `--message-background-hover` (12%).
pub fn hover() -> Rgba {
    overlay(0x1f)
}

/// `--interactive-background-selected` (20%).
pub fn selected() -> Rgba {
    overlay(0x33)
}

/// `--interactive-background-active`, also `--control-secondary-background-hover` (16%).
pub fn active() -> Rgba {
    overlay(0x29)
}

/// `--border-normal`: menus, search fields and cards. Its value is not in the
/// bundle downloaded for this build (JS only); 30% matches the screenshots'
/// `#dfdfe1` over white.
pub fn border_normal() -> Rgba {
    overlay(0x4d)
}

/// `--border-subtle`, also `--app-frame-border` (12%).
pub fn border() -> Rgba {
    overlay(0x1f)
}

pub fn text() -> Hsla {
    rgb(text_default()).into()
}

pub fn muted() -> Hsla {
    rgb(text_muted()).into()
}

/// Mention pill: `--mention-foreground` on the brand color at 30%.
pub fn mention_text() -> u32 {
    pick(0xc9cdfb, 0x2e3c88)
}

pub fn mention_bg() -> Rgba {
    rgba(0x5865f24d)
}

/// `--background-mentioned` behind a message that pings the user, its
/// hover shade, and the bar on its left edge.
pub fn mentioned() -> Rgba {
    rgba(0xf0b2321a)
}

pub fn mentioned_hover() -> Rgba {
    rgba(0xf0b23229)
}

pub fn mentioned_bar() -> u32 {
    0xf0b232
}

/// A message the user jumped to, flashed like `--background-message-highlight`.
pub fn jumped() -> Rgba {
    rgba(0x5865f233)
}

/// Text selected in a message.
pub fn text_selection() -> Rgba {
    rgba(0x5865f266)
}

/// Background of a reaction we added: the brand color at 15%.
pub fn reaction_mine() -> Rgba {
    rgba(0x5865f226)
}

/// Inline code and spoiler background: `--background-code`.
pub fn code_bg() -> Rgba {
    rgba(pick(0x1e1f2299, 0x5865f20a))
}

/// A text spoiler still hidden: `--spoiler-hidden-background`, opaque so
/// the text under it cannot be read.
pub fn spoiler_hidden() -> u32 {
    pick(0x1e1f22, 0x8f9097)
}
