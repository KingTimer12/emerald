//! The suggestion list above the message box.

use super::*;

impl Emerald {
    /// The list, above the message box.
    pub(in super::super) fn render_suggestions(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.autocomplete.is_open() {
            return None;
        }
        let selected = self.autocomplete.selected();
        let mut rows: Vec<AnyElement> = Vec::new();
        let mut section = None;
        for (ix, suggestion) in self.suggestions.iter().enumerate() {
            if section != Some(suggestion.section()) {
                section = Some(suggestion.section());
                rows.push(
                    div()
                        .px_2()
                        .pt_2()
                        .pb_1()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::muted())
                        .child(suggestion.section())
                        .into_any_element(),
                );
            }
            rows.push(self.render_suggestion(ix, suggestion, ix == selected, cx));
        }
        Some(
            div()
                .id("suggestions")
                .absolute()
                .left_0()
                .right_0()
                .bottom(relative(1.))
                .mb_2()
                .max_h(px(ROW * 10. + 64.))
                .overflow_y_scroll()
                .p_2()
                .flex()
                .flex_col()
                .rounded_lg()
                .border_1()
                .border_color(theme::border())
                .bg(rgb(theme::SURFACE_HIGH))
                .shadow_lg()
                .occlude()
                .children(rows)
                .into_any_element(),
        )
    }

    pub(super) fn render_suggestion(&self, ix: usize, suggestion: &Suggestion, selected: bool, cx: &mut Context<Self>) -> AnyElement {
        let (leading, name, detail, color): (AnyElement, SharedString, Option<SharedString>, u32) = match suggestion {
            Suggestion::Member { id, name, username, color } => (
                widgets::avatar(name, *id, self.user_avatar(*id), 24., None).into_any_element(),
                name.clone(),
                Some(username.clone()),
                color.unwrap_or(theme::TEXT_STRONG),
            ),
            Suggestion::Role { name, color, .. } => (
                div().w(px(24.)).flex().justify_center().child("@").into_any_element(),
                format!("@{name}").into(),
                None,
                color.unwrap_or(theme::TEXT_STRONG),
            ),
            Suggestion::Everyone(word) => (
                div().w(px(24.)).flex().justify_center().child("@").into_any_element(),
                format!("@{word}").into(),
                Some(if *word == "here" { t!("autocomplete.here") } else { t!("autocomplete.everyone") }.into()),
                theme::TEXT_STRONG,
            ),
            Suggestion::Channel { name, icon, .. } => (
                icons::icon(icon, 20., theme::CHANNELS_DEFAULT).into_any_element(),
                name.clone(),
                None,
                theme::TEXT_STRONG,
            ),
            Suggestion::Emoji { emoji, name } => (
                self.emoji_image(emoji, 24.),
                format!(":{name}:").into(),
                None,
                theme::TEXT_STRONG,
            ),
            Suggestion::Command { command, app } => (
                div().w(px(24.)).flex().justify_center().child("/").into_any_element(),
                format!("/{}", command.name).into(),
                Some(if command.description.is_empty() {
                    app.clone()
                } else {
                    format!("{} · {app}", command.description).into()
                }),
                theme::TEXT_STRONG,
            ),
        };
        div()
            .id(("suggestion", ix))
            .h(px(ROW))
            .flex_none()
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .rounded_md()
            .cursor_pointer()
            .when(selected, |row| row.bg(theme::selected()))
            .child(div().size(px(24.)).flex_none().flex().items_center().justify_center().child(leading))
            .child(
                div()
                    .flex_none()
                    .max_w(relative(0.6))
                    .truncate()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(color))
                    .child(name),
            )
            .children(detail.map(|detail| {
                div().flex_1().min_w(px(0.)).truncate().text_size(px(14.)).text_color(theme::muted()).child(detail)
            }))
            .on_hover(cx.listener(move |this, inside: &bool, _, cx| {
                if *inside && this.autocomplete.selected() != ix {
                    this.autocomplete.select(ix);
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |this, _, _, cx| this.accept_suggestion(ix, cx)))
            .into_any_element()
    }
}
