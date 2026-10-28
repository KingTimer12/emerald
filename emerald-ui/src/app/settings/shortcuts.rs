//! Shortcuts: each one's key, changed by pressing the new keys, or reset to
//! its default.

use gpui::KeyDownEvent;

use super::*;
use crate::shortcuts::{self, Refusal, Shortcut};
use crate::zoom::zoomed;

/// Keys that only change modifiers; a shortcut waits for a key after them.
const MODIFIER_KEYS: [&str; 5] = ["shift", "control", "alt", "platform", "function"];

/// The name of a shortcut as the settings list it.
pub(super) fn label(shortcut: Shortcut) -> &'static str {
    match shortcut {
        Shortcut::QuickSwitch => t!("shortcuts.quick_switch"),
        Shortcut::EditLast => t!("shortcuts.edit_last"),
        Shortcut::MarkRead => t!("shortcuts.mark_read"),
        Shortcut::NextUnread => t!("shortcuts.next_unread"),
        Shortcut::PreviousChannel => t!("shortcuts.previous_channel"),
        Shortcut::NextChannel => t!("shortcuts.next_channel"),
        Shortcut::Quit => t!("shortcuts.quit"),
        Shortcut::Logout => t!("shortcuts.log_out"),
    }
}

impl Emerald {
    pub(super) fn render_shortcuts(&self, view: &SettingsView, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .child(page_title(t!("settings.shortcuts")))
            .child(hint(t!("shortcuts.hint")).mb_2())
            .children(Shortcut::ALL.into_iter().map(|shortcut| self.shortcut_row(shortcut, view, cx)))
            .children(view.feedback.as_ref().map(|f| widgets::feedback(f).mt_4()))
            .into_any_element()
    }

    fn shortcut_row(&self, shortcut: Shortcut, view: &SettingsView, cx: &mut Context<Self>) -> gpui::Div {
        let id = shortcut.id();
        let recording = view.recording == Some(shortcut);
        let custom = self.settings.shortcuts.contains_key(id);
        let keys = if recording {
            t!("shortcuts.press_keys").to_owned()
        } else {
            shortcuts::describe(&shortcut.key(&self.settings.shortcuts))
        };

        div()
            .flex()
            .items_center()
            .gap_3()
            .py_3()
            .border_b_1()
            .border_color(theme::border())
            .child(section_title(label(shortcut)).flex_1().min_w(zoomed(0.)).font_weight(FontWeight::MEDIUM))
            .child(
                div()
                    .px_2()
                    .py(zoomed(4.))
                    .rounded_md()
                    .bg(theme::hover())
                    .text_color(rgb(if recording { theme::brand() } else { theme::text_strong() }))
                    .child(keys),
            )
            .child(
                widgets::button(("shortcut-change", shortcut as usize), t!("shortcuts.change"), ButtonKind::Secondary)
                    .on_click(cx.listener(move |this, _, window, cx| this.start_recording(shortcut, window, cx))),
            )
            .when(custom, |row| {
                row.child(
                    widgets::button(("shortcut-reset", shortcut as usize), t!("shortcuts.reset"), ButtonKind::Secondary)
                        .on_click(cx.listener(move |this, _, _, cx| this.reset_shortcut(shortcut, cx))),
                )
            })
    }

    fn start_recording(&mut self, shortcut: Shortcut, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(view) = self.settings_view() {
            view.recording = Some(shortcut);
            view.feedback = None;
        }
        // Keys go to the window, not to whichever text input had focus.
        window.focus(&self.focus_handle);
        cx.notify();
    }

    /// Takes the next keys pressed while a shortcut waits for one. Escape
    /// alone cancels; a key already used elsewhere is refused.
    pub(in super::super) fn record_shortcut(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let Some(shortcut) = self.settings_view().and_then(|view| view.recording) else { return };
        cx.stop_propagation();
        let stroke = &event.keystroke;
        if MODIFIER_KEYS.contains(&stroke.key.as_str()) {
            return;
        }
        let modifiers = stroke.modifiers;
        let bare_escape = stroke.key == "escape"
            && !(modifiers.control || modifiers.alt || modifiers.shift || modifiers.platform || modifiers.function);
        if let Some(view) = self.settings_view() {
            view.recording = None;
        }
        if bare_escape {
            cx.notify();
            return;
        }

        let result = shortcuts::check(shortcut, &stroke.unparse(), &self.settings.shortcuts);
        let feedback = match result {
            Ok(key) => {
                self.set_shortcut(shortcut, key, cx);
                Feedback::from_result(Ok(()), t!("shortcuts.saved"))
            }
            Err(Refusal::Invalid) => Feedback::from_result(Err(t!("shortcuts.invalid").into()), ""),
            Err(Refusal::Fixed) => Feedback::from_result(Err(t!("shortcuts.fixed").into()), ""),
            Err(Refusal::Taken(other)) => {
                Feedback::from_result(Err(t!("shortcuts.taken", name = label(other))), "")
            }
        };
        if let Some(view) = self.settings_view() {
            view.feedback = Some(feedback);
        }
        cx.notify();
    }

    fn reset_shortcut(&mut self, shortcut: Shortcut, cx: &mut Context<Self>) {
        self.settings.shortcuts.remove(shortcut.id());
        self.apply_shortcuts(cx);
        if let Some(view) = self.settings_view() {
            view.feedback = Some(Feedback::from_result(Ok(()), t!("shortcuts.saved")));
        }
        cx.notify();
    }

    /// Saves `key` for `shortcut`; the default key is saved as no override.
    fn set_shortcut(&mut self, shortcut: Shortcut, key: String, cx: &mut Context<Self>) {
        let is_default = shortcuts::normalize(shortcut.default_key()).as_deref() == Some(key.as_str());
        if is_default {
            self.settings.shortcuts.remove(shortcut.id());
        } else {
            self.settings.shortcuts.insert(shortcut.id().to_owned(), key);
        }
        self.apply_shortcuts(cx);
    }

    /// Binds the keys in use now and saves them.
    fn apply_shortcuts(&mut self, cx: &mut Context<Self>) {
        crate::shortcuts::bind(cx, &self.settings.shortcuts);
        self.save_settings(cx);
    }
}
