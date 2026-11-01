//! What sits beside a name in Discord 2026: the server tag chip and the
//! role icon, and the name itself in its chosen font and color.

use std::sync::Arc;

use gpui::{App, Div, FontWeight, Image, SharedString, div, img, prelude::*, rgb};

use super::super::super::theme::{self, name_fonts};
use super::super::state::profile_theme;
use super::tooltip;
use crate::zoom::zoomed;
use emerald_discord::model::NameStyle;

/// The server tag chip: the badge, then the tag in small semibold letters,
/// on a pill tinted like a hover. `badge` is drawn `badge_px` wide.
pub fn tag_chip(id: impl Into<gpui::ElementId>, tag: &str, badge: Option<Arc<Image>>, badge_px: f32) -> gpui::Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .h(zoomed(18.))
        .px(zoomed(4.))
        .flex()
        .items_center()
        .gap(zoomed(3.))
        .rounded(theme::radius::tag())
        .bg(theme::hover())
        .text_size(zoomed(12.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(theme::text_default()))
        .children(badge.map(|image| img(image).size(zoomed(badge_px)).flex_none()))
        .child(tag.to_owned())
        .tooltip(tooltip(t!("names.server_tag", tag = tag)))
}

/// A role's icon after a name, `px` wide, named by its tooltip.
pub fn role_icon(id: impl Into<gpui::ElementId>, image: Option<Arc<Image>>, emoji: Option<&str>, role: &str, px: f32) -> gpui::Stateful<Div> {
    let shown = match (image, emoji) {
        (Some(image), _) => img(image).size(zoomed(px)).into_any_element(),
        (None, Some(emoji)) => div().text_size(zoomed(px * 0.85)).child(emoji.to_owned()).into_any_element(),
        (None, None) => div().size(zoomed(px)).into_any_element(),
    };
    div()
        .id(id)
        .flex_none()
        .size(zoomed(px))
        .flex()
        .items_center()
        .justify_center()
        .child(shown)
        .tooltip(tooltip(SharedString::from(role.to_owned())))
}

/// `name` in its display-name font, if Emerald has it, and in `color`; a
/// style's own color replaces `color` only where `effects` (outside guilds,
/// as the web client draws them), pushed to stay readable on `background`.
pub fn styled_name(name: impl Into<SharedString>, style: Option<&NameStyle>, color: u32, effects: bool, background: u32, cx: &App) -> Div {
    let font = style.and_then(|s| name_fonts::use_family(s.font_id, cx));
    let color = style
        .filter(|_| effects)
        .and_then(|s| profile_theme::name_color(s, background))
        .unwrap_or(color);
    div()
        .text_color(rgb(color))
        .font_weight(FontWeight::MEDIUM)
        .when_some(font, |name, font| name.font_family(font))
        .child(name.into())
}
