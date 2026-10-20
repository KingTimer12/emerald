//! "Language & Time": the language picker, a searchable list with each
//! language's flag and own name, and the 12/24-hour clock choice, laid out
//! like Discord's page.

use gpui::{AnyElement, Context, Focusable, FontWeight, Window, div, prelude::*, px, rgb};

use super::super::super::icons::{self, icon};
use super::super::super::{theme, time};
use super::super::Emerald;
use super::{SettingsView, page_title, section_title};
use crate::discord::Command;
use crate::i18n::{self, Language};
use crate::settings::TimeFormat;

/// Rows the open picker shows before it scrolls.
const LIST_HEIGHT: f32 = 320.;

impl Emerald {
    pub(super) fn render_language(&self, view: &SettingsView, cx: &mut Context<Self>) -> AnyElement {
        let mut formats = Vec::new();
        for format in TimeFormat::ALL {
            formats.push(self.time_format_row(format, cx));
        }
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(page_title(t!("settings.language_time")))
            .child(section_title(t!("settings.select_language")))
            .child(self.language_picker(view, cx))
            .child(section_title(t!("settings.time_format")).mt_8())
            .children(formats)
            .into_any_element()
    }

    /// The current language as a dropdown; open, a search box over the list.
    fn language_picker(&self, view: &SettingsView, cx: &mut Context<Self>) -> impl IntoElement {
        let current = i18n::current();
        let shown = i18n::available().find(|l| l.code == current);
        let trigger = div()
            .id("language-picker")
            .h(px(44.))
            .px_3()
            .flex()
            .items_center()
            .gap_2()
            .rounded_lg()
            .border_1()
            .border_color(if view.language_open { rgb(theme::BRAND) } else { theme::border() })
            .bg(rgb(theme::BASE_LOWEST))
            .cursor_pointer()
            .children(shown.map(language_label))
            .child(div().flex_1())
            .child(icon(icons::CHEVRON, 16., theme::INTERACTIVE))
            .on_click(cx.listener(|this, _, window, cx| this.toggle_language_picker(window, cx)));

        let list = view.language_open.then(|| {
            let query = view.language_query.read(cx).text().trim().to_lowercase();
            let matches: Vec<Language> = i18n::available().filter(|l| matches(l, &query)).collect();
            div()
                .flex()
                .flex_col()
                .gap_1()
                .p_2()
                .rounded_lg()
                .border_1()
                .border_color(theme::border())
                .bg(rgb(theme::BASE_LOWEST))
                .child(view.language_query.clone())
                .child(
                    div()
                        .id("language-list")
                        .max_h(px(LIST_HEIGHT))
                        .overflow_y_scroll()
                        .flex()
                        .flex_col()
                        .children(matches.into_iter().enumerate().map(|(ix, language)| {
                            let selected = language.code == current;
                            div()
                                .id(("language", ix))
                                .h(px(40.))
                                .px_2()
                                .flex()
                                .items_center()
                                .gap_2()
                                .rounded_md()
                                .cursor_pointer()
                                .when(selected, |row| row.bg(theme::selected()))
                                .when(!selected, |row| row.hover(|s| s.bg(theme::hover())))
                                .child(language_label(language))
                                .child(div().flex_1())
                                .when(selected, |row| row.child(div().text_color(rgb(theme::BRAND)).child("✓")))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.pick_language(language.code, window, cx)
                                }))
                        })),
                )
        });

        div().flex().flex_col().gap_2().child(trigger).children(list)
    }

    fn toggle_language_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(view) = self.settings_view() else { return };
        view.language_open = !view.language_open;
        if view.language_open {
            view.language_query.update(cx, |input, cx| input.set_text("", cx));
            window.focus(&view.language_query.focus_handle(cx));
        }
        cx.notify();
    }

    fn pick_language(&mut self, code: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(view) = self.settings_view() {
            view.language_open = false;
        }
        if code != i18n::current() {
            self.settings.language = Some(code.to_owned());
            self.save_settings(cx);
            self.apply_language(code, window, cx);
        }
        cx.notify();
    }

    /// Switches the interface language in place. Most text is looked up as
    /// views are built; the rest was worded ahead of time and is redone here.
    fn apply_language(&mut self, code: &str, window: &mut Window, cx: &mut Context<Self>) {
        i18n::set(code);
        self.net.post(Command::SetLocale(code.to_owned()));
        let placeholders = [
            (&self.dm_search, t!("dm.search_placeholder")),
            (&self.member_query, t!("members.search")),
            (&self.friends.input, t!("friends.username_placeholder")),
            (&self.composer.input, t!("composer.no_channel")),
        ];
        for (input, text) in placeholders {
            input.update(cx, |input, cx| input.set_placeholder(text, cx));
        }
        if let Some(view) = self.settings_view() {
            let inputs = [
                (view.display_name.clone(), t!("settings.display_name_placeholder")),
                (view.language_query.clone(), t!("settings.search_languages")),
            ];
            for (input, text) in inputs {
                input.update(cx, |input, cx| input.set_placeholder(text, cx));
            }
        }
        self.connection.relabel();
        self.spaces.relabel();
        // Also words the composer for the open channel again.
        self.reload_messages(window, cx);
    }

    fn time_format_row(&self, format: TimeFormat, cx: &mut Context<Self>) -> AnyElement {
        let selected = self.settings.time_format == format;
        let label = match format {
            TimeFormat::Auto => t!("settings.time_auto"),
            TimeFormat::Hours12 => t!("settings.time_12h"),
            TimeFormat::Hours24 => t!("settings.time_24h"),
        };
        div()
            .id(("time-format", format as usize))
            .flex()
            .items_center()
            .gap_3()
            .px_3()
            .py_2p5()
            .rounded_lg()
            .cursor_pointer()
            .bg(if selected { theme::selected() } else { theme::hover() })
            .when(!selected, |row| row.hover(|s| s.bg(theme::active())))
            .child(radio(selected))
            .child(
                div()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(theme::TEXT_STRONG))
                    .child(label),
            )
            .on_click(cx.listener(move |this, _, window, cx| this.set_time_format(format, window, cx)))
            .into_any_element()
    }

    /// Applies at once: the messages on screen are rebuilt with the new clock.
    fn set_time_format(&mut self, format: TimeFormat, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.time_format == format {
            return;
        }
        self.settings.time_format = format;
        time::set_time_format(format);
        self.save_settings(cx);
        self.reload_messages(window, cx);
    }

    /// Messages carry their timestamps already worded: refetches the open
    /// channel and lets the others reload when visited.
    fn reload_messages(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.history.clear();
        if let Some(channel) = self.selected_channel.filter(|_| !self.friends_open) {
            self.select_channel(channel, window, cx);
        }
    }
}

/// Matches the language's own name or its code.
fn matches(language: &Language, query: &str) -> bool {
    query.is_empty() || language.name.to_lowercase().contains(query) || language.code.to_lowercase().contains(query)
}

fn language_label(language: Language) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(div().w(px(24.)).text_size(px(18.)).child(language.flag))
        .child(div().text_color(rgb(theme::TEXT_STRONG)).child(language.name))
}

/// Discord's radio button: a ring, filled in the brand color when chosen.
fn radio(selected: bool) -> impl IntoElement {
    div()
        .size(px(20.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .border_2()
        .border_color(rgb(if selected { theme::BRAND } else { theme::INTERACTIVE }))
        .when(selected, |ring| ring.child(div().size(px(10.)).rounded_full().bg(rgb(theme::BRAND))))
}
