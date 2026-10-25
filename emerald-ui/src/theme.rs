//! Discord's dark theme (2025 visual refresh), resolved from the `.theme-dark`
//! design tokens in the web client's CSS on 2026-10-19 with the default
//! saturation factor and no custom theme. Names follow the CSS variables.

use gpui::{Hsla, Rgba, rgb, rgba};

/// `--background-base-lowest`: server list and channel sidebar.
pub const BASE_LOWEST: u32 = 0x2c2d32;
/// `--background-base-lower`: chat column.
pub const BASE_LOWER: u32 = 0x323339;
/// `--background-base-low`: server icons without an image.
pub const BASE_LOW: u32 = 0x36373e;
/// `--chat-background-default`: message box and the user panel surface.
pub const SURFACE_HIGH: u32 = 0x393a41;
/// `--background-brand`.
pub const BRAND: u32 = 0x5865f2;
/// `--status-danger`.
pub const DANGER: u32 = 0xf23f43;
/// `--status-online`.
pub const ONLINE: u32 = 0x23a55a;
/// `--text-default`.
pub const TEXT_DEFAULT: u32 = 0xf3f3f4;
/// `--text-strong`, also `--interactive-text-hover` / `-active`.
pub const TEXT_STRONG: u32 = 0xffffff;
/// `--text-muted`.
pub const TEXT_MUTED: u32 = 0xabacb2;
/// `--interactive-text-default`.
pub const INTERACTIVE: u32 = 0xc5c6ca;
/// `--channels-default`, also `--channel-icon`.
pub const CHANNELS_DEFAULT: u32 = 0x999aa1;
/// `--interactive-muted`: channels the user cannot read.
pub const INTERACTIVE_MUTED: u32 = 0x4e5058;
/// `--channel-text-area-placeholder`.
pub const PLACEHOLDER: u32 = 0x6d6f78;
/// `--text-link`.
pub const LINK: u32 = 0x76aff6;
/// `--text-feedback-positive` / `--text-feedback-critical`.
pub const FEEDBACK_POSITIVE: u32 = 0x7ecb94;
pub const FEEDBACK_CRITICAL: u32 = 0xffa09b;
/// `--control-primary-background-default` / `-hover`.
pub const BUTTON_BRAND_HOVER: u32 = 0x4452bb;
/// `--control-connected-background-default` / `-hover`.
pub const BUTTON_POSITIVE: u32 = 0x008545;
pub const BUTTON_POSITIVE_HOVER: u32 = 0x006c37;
/// `--control-critical-primary-background-default` / `-hover`, also
/// `--background-feedback-notification` (mention badges).
pub const BUTTON_DANGER: u32 = 0xd22d39;
pub const BUTTON_DANGER_HOVER: u32 = 0xa9232e;
/// Presence colors; idle and offline are not in `.theme-dark`, these are
/// Discord's long-standing values.
pub const IDLE: u32 = 0xf0b232;
pub const OFFLINE: u32 = 0x80848e;

/// `--background-scrim`: black at 72%.
pub fn scrim() -> Rgba {
    rgba(0x000000b8)
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

/// `--border-subtle`, also `--app-frame-border` (12%).
pub fn border() -> Rgba {
    overlay(0x1f)
}

pub fn text() -> Hsla {
    rgb(TEXT_DEFAULT).into()
}

pub fn muted() -> Hsla {
    rgb(TEXT_MUTED).into()
}

/// Mention pill: `--mention-foreground` on the brand color at 30%.
pub const MENTION_TEXT: u32 = 0xc9cdfb;
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
pub const MENTIONED_BAR: u32 = 0xf0b232;

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

/// Inline code and spoiler background.
pub fn code_bg() -> Rgba {
    rgba(0x1e1f2299)
}

/// A text spoiler still hidden: `--spoiler-hidden-background`, opaque so
/// the text under it cannot be read.
pub const SPOILER_HIDDEN: u32 = 0x1e1f22;

/// Code blocks: the system's monospace face, as the web client falls back
/// to when its own font is missing.
#[cfg(target_os = "macos")]
pub const MONO: &str = "Menlo";
#[cfg(not(target_os = "macos"))]
pub const MONO: &str = "Consolas";
