//! The custom status on the Status page: an emoji, a line of text and when
//! it clears itself, as the web client's "Set a custom status" offers.

use super::*;
use crate::time;
use emerald_discord::model::CustomStatus;
use crate::zoom::zoomed;

/// Longest custom status Discord takes.
const MAX_TEXT: usize = 128;

/// Emoji offered with one click; any other can be typed or pasted.
const QUICK_EMOJI: [&str; 8] = ["😀", "😴", "🎮", "📚", "💻", "🎧", "🔥", "❤️"];

/// "Clear after", in the web client's order.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ClearAfter {
    Today,
    Hours4,
    Hour1,
    Minutes30,
    Never,
}

impl ClearAfter {
    const ALL: [ClearAfter; 5] = [Self::Today, Self::Hours4, Self::Hour1, Self::Minutes30, Self::Never];

    fn label(self) -> &'static str {
        match self {
            Self::Today => t!("custom_status.today"),
            Self::Hours4 => t!("custom_status.hours_4"),
            Self::Hour1 => t!("custom_status.hour_1"),
            Self::Minutes30 => t!("custom_status.minutes_30"),
            Self::Never => t!("custom_status.never"),
        }
    }

    /// Unix milliseconds it ends at; 0 for never.
    fn expires_at_ms(self, now_secs: i64) -> u64 {
        let end = match self {
            Self::Today => time::end_of_day(now_secs),
            Self::Hours4 => now_secs + 4 * 3600,
            Self::Hour1 => now_secs + 3600,
            Self::Minutes30 => now_secs + 1800,
            Self::Never => return 0,
        };
        end.max(0) as u64 * 1000
    }
}

pub(super) struct CustomStatusForm {
    text: Entity<TextInput>,
    emoji: Entity<TextInput>,
    clear_after: ClearAfter,
}

impl CustomStatusForm {
    pub(super) fn new(cx: &mut Context<Emerald>) -> Self {
        Self {
            text: cx.new(|cx| TextInput::new(t!("custom_status.placeholder"), cx).keep_on_submit()),
            emoji: cx.new(|cx| TextInput::new("🙂", cx).keep_on_submit()),
            clear_after: ClearAfter::Today,
        }
    }
}

impl Emerald {
    /// Puts the current custom status in the form.
    pub(super) fn fill_custom_status(&mut self, cx: &mut Context<Self>) {
        let current = self.account.custom_status(super::super::account::now_ms());
        let Some(view) = self.settings_view() else { return };
        let form = &mut view.custom_status;
        let (text, emoji) = match &current {
            // A custom emoji cannot be typed here; its name stands in.
            Some(c) => (c.text.clone(), c.emoji_name.clone().filter(|_| c.emoji_id.is_none()).unwrap_or_default()),
            None => (String::new(), String::new()),
        };
        form.text.update(cx, |input, cx| input.set_text(text, cx));
        form.emoji.update(cx, |input, cx| input.set_text(emoji, cx));
        form.clear_after = if current.is_some_and(|c| c.expires_at_ms == 0) { ClearAfter::Never } else { ClearAfter::Today };
    }

    fn save_custom_status(&mut self, cx: &mut Context<Self>) {
        let Some(view) = self.settings_view() else { return };
        let form = &view.custom_status;
        let text = form.text.read(cx).text().trim().to_owned();
        let emoji = form.emoji.read(cx).text().trim().to_owned();
        if text.chars().count() > MAX_TEXT {
            view.feedback = Some(Feedback::from_result(Err(t!("custom_status.too_long").into()), ""));
            cx.notify();
            return;
        }
        let custom = CustomStatus {
            text,
            emoji_id: None,
            emoji_name: (!emoji.is_empty()).then_some(emoji),
            expires_at_ms: form.clear_after.expires_at_ms(time::now_secs()),
        };
        view.feedback = None;
        self.set_custom_status(Some(custom), cx);
    }

    fn clear_custom_status(&mut self, cx: &mut Context<Self>) {
        if let Some(view) = self.settings_view() {
            view.custom_status.text.update(cx, |input, cx| input.set_text("", cx));
            view.custom_status.emoji.update(cx, |input, cx| input.set_text("", cx));
        }
        self.set_custom_status(None, cx);
    }

    pub(super) fn render_custom_status(&self, view: &SettingsView, cx: &mut Context<Self>) -> AnyElement {
        let form = &view.custom_status;
        let busy = self.busy.contains(&Action::AccountSettings);
        let current = self.account.custom_status(super::super::account::now_ms());
        let chip = |id: (&'static str, usize), label: SharedString, selected: bool| {
            div()
                .id(id)
                .h(zoomed(32.))
                .px_3()
                .flex()
                .items_center()
                .rounded_md()
                .text_size(zoomed(14.))
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .text_color(rgb(theme::text_strong()))
                .when(selected, |chip| chip.bg(rgb(theme::brand())))
                .when(!selected, |chip| chip.bg(theme::hover()).hover(|s| s.bg(theme::active())))
                .child(label)
        };

        div()
            .mt_10()
            .flex()
            .flex_col()
            .gap_3()
            .child(section_title(t!("custom_status.title")))
            .child(hint(match &current {
                Some(c) => SharedString::from(t!("custom_status.current", status = c.line())),
                None => t!("custom_status.none").into(),
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(div().w(zoomed(64.)).flex_none().child(form.emoji.clone()))
                    .child(div().flex_1().child(form.text.clone())),
            )
            .child(div().flex().flex_wrap().gap_1().children(QUICK_EMOJI.iter().enumerate().map(|(ix, emoji)| {
                chip(("quick-emoji", ix), (*emoji).into(), false).on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(view) = this.settings_view() {
                        view.custom_status.emoji.update(cx, |input, cx| input.set_text(*emoji, cx));
                    }
                }))
            })))
            .child(widgets::label(t!("custom_status.clear_after")).mt_2())
            .child(div().flex().flex_wrap().gap_2().children(ClearAfter::ALL.into_iter().enumerate().map(|(ix, option)| {
                chip(("clear-after", ix), option.label().into(), form.clear_after == option).on_click(cx.listener(
                    move |this, _, _, cx| {
                        if let Some(view) = this.settings_view() {
                            view.custom_status.clear_after = option;
                        }
                        cx.notify();
                    },
                ))
            })))
            .child(
                div()
                    .mt_2()
                    .flex()
                    .gap_3()
                    .child(
                        widgets::button(
                            "save-custom-status",
                            if busy { t!("common.saving") } else { t!("common.save") },
                            ButtonKind::Brand,
                        )
                        .when(busy, |b| b.opacity(0.5))
                        .on_click(cx.listener(|this, _, _, cx| this.save_custom_status(cx))),
                    )
                    .when(current.is_some(), |row| {
                        row.child(
                            widgets::button("clear-custom-status", t!("custom_status.clear"), ButtonKind::Secondary)
                                .on_click(cx.listener(|this, _, _, cx| this.clear_custom_status(cx))),
                        )
                    }),
            )
            .children(view.feedback.as_ref().map(widgets::feedback))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clearing_times_count_from_now() {
        assert_eq!(ClearAfter::Never.expires_at_ms(1000), 0);
        assert_eq!(ClearAfter::Hour1.expires_at_ms(1000), 4_600_000);
        let today = ClearAfter::Today.expires_at_ms(1_700_000_000);
        assert!(today > 1_700_000_000_000 && today <= 1_700_086_400_000);
    }
}
