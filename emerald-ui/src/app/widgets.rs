//! Small Discord-styled building blocks shared by the views.

use std::sync::Arc;

use gpui::{
    AnyView, App, Context, Div, ElementId, FontWeight, Image, SharedString, Stateful, Window, div,
    img, prelude::*, px, rgb,
};

use super::super::{icons, theme};
use super::Feedback;
use emerald_discord::model::Snowflake;

/// Discord's default avatar colors, picked by user id.
const AVATAR_COLORS: [u32; 6] = [0x5865f2, 0x3ba55c, 0xfaa61a, 0xed4245, 0xeb459e, 0x747f8d];

pub fn status_color(status: &str) -> u32 {
    match status {
        "online" => theme::ONLINE,
        "idle" => theme::IDLE,
        "dnd" => theme::DANGER,
        _ => theme::OFFLINE,
    }
}

/// The labels the web client uses for each presence.
pub fn status_label(status: Option<&str>) -> &'static str {
    match status {
        Some("online") => t!("status.online"),
        Some("idle") => t!("status.idle"),
        Some("dnd") => t!("status.dnd"),
        Some("invisible") => t!("status.invisible"),
        _ => t!("status.offline"),
    }
}

/// Round avatar: the downloaded `image` when there is one, else the initial
/// on a colored disc (also shown while the image loads). `status` adds the
/// presence dot, ringed with the background color it sits on.
pub fn avatar(
    name: &str,
    id: Snowflake,
    image: Option<Arc<Image>>,
    size: f32,
    status: Option<(&str, u32)>,
) -> Div {
    let initial: SharedString = name
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_default()
        .into();
    let color = AVATAR_COLORS[(id.0 % AVATAR_COLORS.len() as u64) as usize];
    // Discord's dot: 10px on small avatars, a quarter of large ones.
    let dot = (size * 0.25).max(10.);

    div()
        .relative()
        .flex_none()
        .size(px(size))
        .rounded_full()
        .bg(rgb(color))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px((size * 0.45).round()))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(theme::TEXT_STRONG))
        .map(|avatar| match image {
            // The id keeps an animated avatar's frame between redraws;
            // still ones cost nothing more.
            Some(image) => avatar.child(img(image).id(("avatar", id.0)).size_full().rounded_full()),
            None => avatar.child(initial),
        })
        .when_some(status, |avatar, (status, ring)| {
            avatar.child(
                div()
                    .absolute()
                    .right(px(-2.))
                    .bottom(px(-2.))
                    .size(px(dot))
                    .rounded_full()
                    .border_2()
                    .border_color(rgb(ring))
                    .bg(rgb(status_color(status)))
                    .flex()
                    .items_center()
                    .justify_center()
                    // Offline is a hollow ring, as on Discord.
                    .when(!matches!(status, "online" | "idle" | "dnd"), |ring_dot| {
                        ring_dot.child(div().size(px(((dot - 4.) * 0.5).max(2.))).rounded_full().bg(rgb(ring)))
                    }),
            )
        })
}

/// Red count pill, as on pending friend requests.
pub fn badge(count: usize) -> Div {
    div()
        .flex_none()
        .min_w(px(16.))
        .h(px(16.))
        .px_1()
        .rounded_full()
        .bg(rgb(theme::BUTTON_DANGER))
        .flex()
        .items_center()
        .justify_center()
        .text_xs()
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(theme::TEXT_STRONG))
        .child(count.to_string())
}

#[derive(Clone, Copy)]
pub enum ButtonKind {
    Brand,
    Secondary,
    Positive,
    Danger,
}

pub fn button(id: impl Into<ElementId>, label: impl Into<SharedString>, kind: ButtonKind) -> Stateful<Div> {
    let (bg, hover) = match kind {
        ButtonKind::Brand => (rgb(theme::BRAND), rgb(theme::BUTTON_BRAND_HOVER)),
        ButtonKind::Secondary => (theme::hover(), theme::active()),
        ButtonKind::Positive => (rgb(theme::BUTTON_POSITIVE), rgb(theme::BUTTON_POSITIVE_HOVER)),
        ButtonKind::Danger => (rgb(theme::BUTTON_DANGER), rgb(theme::BUTTON_DANGER_HOVER)),
    };
    div()
        .id(id)
        .flex_none()
        .h(px(38.))
        .px_4()
        .flex()
        .items_center()
        .justify_center()
        .rounded_lg()
        .bg(bg)
        .text_size(px(14.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(theme::TEXT_STRONG))
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .child(label.into())
}

/// Plain text button, like Discord's "Back" and "Cancel".
pub fn link_button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .px_2()
        .py_1()
        .text_size(px(14.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(theme::INTERACTIVE))
        .cursor_pointer()
        .hover(|s| s.text_color(rgb(theme::TEXT_STRONG)))
        .child(label.into())
}

pub fn icon_button(id: impl Into<ElementId>, glyph: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .size(px(32.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .text_size(px(18.))
        .text_color(rgb(theme::INTERACTIVE))
        .cursor_pointer()
        .hover(|s| s.bg(theme::hover()).text_color(rgb(theme::TEXT_STRONG)))
        .child(glyph)
}

/// [`icon_button`] with an SVG icon. Svg colors are not inherited, so the
/// hover color is restated on the icon through the button's group.
pub fn svg_button(id: &'static str, path: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .group(id)
        .flex_none()
        .size(px(32.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .cursor_pointer()
        .hover(|s| s.bg(theme::hover()))
        .child(
            icons::icon(path, 20., theme::INTERACTIVE)
                .group_hover(id, |s| s.text_color(rgb(theme::TEXT_STRONG))),
        )
}

/// Uppercase field caption above inputs and values.
pub fn label(text: impl Into<SharedString>) -> Div {
    div()
        .text_xs()
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(theme::TEXT_MUTED))
        .child(text.into())
}

pub fn feedback(feedback: &Feedback) -> Div {
    div()
        .text_size(px(14.))
        .text_color(rgb(if feedback.ok {
            theme::FEEDBACK_POSITIVE
        } else {
            theme::FEEDBACK_CRITICAL
        }))
        .child(feedback.text.clone())
}

/// On/off switch with a sliding knob.
pub fn toggle(id: impl Into<ElementId>, on: bool) -> Stateful<Div> {
    div()
        .id(id)
        .relative()
        .flex_none()
        .w(px(40.))
        .h(px(24.))
        .rounded_full()
        .bg(rgb(if on { theme::BRAND } else { theme::INTERACTIVE_MUTED }))
        .cursor_pointer()
        .child(
            div()
                .absolute()
                .top(px(3.))
                .left(px(if on { 19. } else { 3. }))
                .size(px(18.))
                .rounded_full()
                .bg(rgb(theme::TEXT_STRONG)),
        )
}

/// Hover label, built through `.tooltip(widgets::tooltip("…"))`.
pub struct Tooltip(SharedString);

impl Render for Tooltip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_3()
            .py(px(6.))
            .rounded_lg()
            .border_1()
            .border_color(theme::border())
            .bg(rgb(theme::SURFACE_HIGH))
            .text_size(px(14.))
            .line_height(px(18.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(theme::TEXT_DEFAULT))
            .child(self.0.clone())
    }
}

pub fn tooltip(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text = text.into();
    move |_, cx| cx.new(|_| Tooltip(text.clone())).into()
}
