//! Appearance: message size and density.

use super::*;

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
            .bg(rgb(theme::BASE_LOWER))
            .text_size(px(size))
            .line_height(px((size * 1.375).round()))
            .child(if compact {
                div()
                    .flex()
                    .gap_2()
                    .child(div().text_xs().text_color(theme::muted()).child("12:00"))
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(theme::TEXT_STRONG))
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
                                    .text_color(rgb(theme::TEXT_STRONG))
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
                    let selected = option == size;
                    div()
                        .id(("font-size", ix))
                        .h(px(32.))
                        .px_3()
                        .flex()
                        .items_center()
                        .rounded_md()
                        .text_size(px(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .cursor_pointer()
                        .text_color(rgb(theme::TEXT_STRONG))
                        .when(selected, |chip| chip.bg(rgb(theme::BRAND)))
                        .when(!selected, |chip| chip.bg(theme::hover()).hover(|s| s.bg(theme::active())))
                        .child(format!("{option}px"))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.settings.font_size = option;
                            this.save_settings(cx);
                        }))
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
                                    .text_size(px(14.))
                                    .text_color(rgb(theme::TEXT_MUTED))
                                    .child(t!("settings.compact_hint")),
                            ),
                    )
                    .child(widgets::toggle("compact-toggle", compact))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.settings.compact = !this.settings.compact;
                        this.save_settings(cx);
                    })),
            )
            .into_any_element()
    }
}
