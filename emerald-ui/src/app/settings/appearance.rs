//! Appearance: message size, zoom, theme, density and motion.

use super::*;
use crate::zoom::{self, zoomed};

impl Emerald {
    pub(super) fn render_appearance(&self, cx: &mut Context<Self>) -> AnyElement {
        let size = self.settings.font_size;
        let compact = self.settings.compact;
        let preview_text = t!("settings.preview_text");
        let preview = div()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(theme::border())
            .bg(rgb(theme::base_lower()))
            .text_size(zoomed(size))
            .line_height(zoomed((size * 1.375).round()))
            .child(if compact {
                div()
                    .flex()
                    .gap_2()
                    .child(div().text_xs().text_color(theme::muted()).child("12:00"))
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(theme::text_strong()))
                            .child("Wumpus"),
                    )
                    .child(div().child(preview_text))
            } else {
                div()
                    .flex()
                    .gap_4()
                    .child(widgets::avatar(
                        "Wumpus",
                        Snowflake(0),
                        self.avatar_image(&avatar_url(Snowflake(0), None)),
                        40.,
                        None,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(theme::text_strong()))
                                    .child("Wumpus"),
                            )
                            .child(div().child(preview_text)),
                    )
            });

        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(page_title(t!("settings.appearance")))
            .child(preview)
            .child(section_title(t!("settings.font_size")).mt_6())
            .child(
                div().flex().gap_2().children(FONT_SIZES.into_iter().enumerate().map(|(ix, option)| {
                    widgets::chip(("font-size", ix), format!("{option}px"), option == size).on_click(cx.listener(
                        move |this, _, _, cx| {
                            this.settings.font_size = option;
                            this.save_settings(cx);
                        },
                    ))
                })),
            )
            .child(section_title(t!("settings.zoom")).mt_6())
            .child(hint(t!("settings.zoom_hint")))
            .child(
                div().flex().gap_2().children(zoom::LEVELS.into_iter().enumerate().map(|(ix, option)| {
                    widgets::chip(("zoom-level", ix), format!("{option}%"), option == self.settings.zoom).on_click(
                        cx.listener(move |this, _, _, cx| this.set_zoom(option, cx)),
                    )
                })),
            )
            .child(
                div()
                    .id("compact-mode")
                    .mt_6()
                    .flex()
                    .items_center()
                    .gap_4()
                    .cursor_pointer()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .child(section_title(t!("settings.compact")))
                            .child(
                                div()
                                    .text_size(zoomed(14.))
                                    .text_color(rgb(theme::text_muted()))
                                    .child(t!("settings.compact_hint")),
                            ),
                    )
                    .child(widgets::toggle("compact-toggle", compact))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.settings.compact = !this.settings.compact;
                        this.save_settings(cx);
                    })),
            )
            .child(section_title(t!("settings.theme")).mt_6())
            .child(
                div().flex().gap_2().children(ThemeChoice::ALL.into_iter().enumerate().map(|(ix, option)| {
                    let label = match option {
                        ThemeChoice::Dark => t!("settings.theme_dark"),
                        ThemeChoice::Light => t!("settings.theme_light"),
                        ThemeChoice::System => t!("settings.theme_system"),
                    };
                    widgets::chip(("theme-choice", ix), label, option == self.settings.theme).on_click(cx.listener(
                        move |this, _, _, cx| {
                            this.settings.theme = option;
                            this.save_settings(cx);
                            cx.notify();
                        },
                    ))
                })),
            )
            .child(
                switch_row(
                    "reduced-motion",
                    t!("settings.reduced_motion"),
                    t!("settings.reduced_motion_hint"),
                    self.settings.reduced_motion,
                    true,
                )
                .mt_6()
                .border_b_0()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.settings.reduced_motion = !this.settings.reduced_motion;
                    this.save_settings(cx);
                })),
            )
            .into_any_element()
    }
}

