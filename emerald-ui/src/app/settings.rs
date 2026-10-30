//! Full-screen user settings, laid out like Discord's: category sidebar on
//! the left, the open page in the middle and the ESC close button on the right.

use gpui::{
    AnyElement, App, AppContext, Context, Entity, FontWeight, SharedString, Subscription, Window, div, prelude::*,
    rgb,
};

use super::super::text_input::{Submitted, TextInput};
use super::super::theme;
use super::widgets::{self, ButtonKind};
use super::{Emerald, Feedback, Overlay};
use emerald_discord::model::{Snowflake, Status, avatar_url};
use emerald_discord::{Action, Command};
use crate::settings::{FONT_SIZES, ThemeChoice};
use crate::zoom::zoomed;

mod account;
mod accounts;
mod appearance;
mod custom_status;
mod desktop;
mod devices;
mod language;
mod privacy;
mod profiles;
mod shortcuts;
mod voice;

pub(super) use profiles::ProfileForm;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Account,
    Profiles,
    Privacy,
    Devices,
    Accounts,
    Status,
    Appearance,
    Voice,
    Shortcuts,
    Language,
    Desktop,
}

pub struct SettingsView {
    page: Page,
    display_name: Entity<TextInput>,
    reveal_email: bool,
    pub feedback: Option<super::Feedback>,
    /// The language picker's search box, and whether its list is open.
    language_query: Entity<TextInput>,
    language_open: bool,
    /// The profile being edited, made when Profiles opens.
    pub profile: Option<ProfileForm>,
    custom_status: custom_status::CustomStatusForm,
    /// A privacy default just changed, asking whether it applies to the
    /// servers already joined.
    privacy_ask: Option<privacy::Ask>,
    /// The accounts signed in here, read when Accounts opens.
    accounts: emerald_discord::token::Accounts,
    /// The shortcut waiting for its new keys, if any.
    recording: Option<crate::shortcuts::Shortcut>,
    /// Microphones and speakers, listed when Voice opens.
    audio_devices: Option<emerald_audio::Devices>,
    _subscriptions: Vec<Subscription>,
}

impl Emerald {
    pub(super) fn open_settings(&mut self, page: Page, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.overlay, Some(Overlay::Settings(_))) {
            self.create_settings(window, cx);
        }
        if let Some(view) = self.settings_view() {
            view.page = page;
            view.feedback = None;
            view.privacy_ask = None;
            view.recording = None;
        }
        match page {
            Page::Profiles => self.open_profile_form(None, cx),
            Page::Devices => self.fetch_sessions(),
            Page::Accounts => {
                if let Some(view) = self.settings_view() {
                    view.accounts = emerald_discord::token::Accounts::load();
                }
            }
            Page::Status => self.fill_custom_status(cx),
            Page::Voice => self.list_audio_devices(cx),
            Page::Desktop => self.opens_on_login = self.desktop.opens_on_login(),
            _ => {}
        }
        cx.notify();
    }

    fn create_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = self
            .people
            .me()
            .and_then(|me| me.global_name.clone())
            .unwrap_or_default();
        let display_name = cx.new(|cx| {
            let mut input = TextInput::new(t!("settings.display_name_placeholder"), cx).keep_on_submit();
            input.set_text(current, cx);
            input
        });
        let language_query = cx.new(|cx| TextInput::new(t!("settings.search_languages"), cx).keep_on_submit().compact());
        let subscriptions = vec![
            cx.subscribe(&display_name, |this, _, event: &Submitted, cx| {
                this.save_display_name(event.0.clone(), cx)
            }),
            cx.observe(&language_query, |_, _, cx| cx.notify()),
        ];
        let custom_status = custom_status::CustomStatusForm::new(cx);
        self.overlay = Some(Overlay::Settings(Box::new(SettingsView {
            page: Page::Account,
            display_name,
            reveal_email: false,
            feedback: None,
            language_query,
            language_open: false,
            profile: None,
            custom_status,
            privacy_ask: None,
            accounts: Default::default(),
            recording: None,
            audio_devices: None,
            _subscriptions: subscriptions,
        })));
        window.focus(&self.focus_handle);
    }

    pub(super) fn settings_view(&mut self) -> Option<&mut SettingsView> {
        match &mut self.overlay {
            Some(Overlay::Settings(view)) => Some(view),
            _ => None,
        }
    }

    pub(super) fn render_settings(&self, view: &SettingsView, cx: &mut Context<Self>) -> impl IntoElement {
        let item = |id: &'static str, label: &'static str, page: Page, cx: &mut Context<Self>| {
            let selected = view.page == page;
            div()
                .id(id)
                .px_2p5()
                .py(zoomed(6.))
                .rounded(theme::radius::control())
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .text_color(rgb(if selected { theme::text_strong() } else { theme::interactive() }))
                .when(selected, |i| i.bg(theme::selected()))
                .when(!selected, |i| {
                    i.hover(|s| s.bg(theme::hover()).text_color(rgb(theme::text_strong())))
                })
                .child(label)
                .on_click(cx.listener(move |this, _, window, cx| this.open_settings(page, window, cx)))
        };
        let separator = || div().mx_2p5().my_2().h(zoomed(1.)).bg(theme::border());

        let sidebar = div()
            .id("settings-sidebar")
            .w(zoomed(260.))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .gap(zoomed(2.))
            .pt(zoomed(60.))
            .px_3()
            .overflow_y_scroll()
            .bg(rgb(theme::base_lowest()))
            .child(widgets::label(t!("settings.user_settings")).px_2p5().pb_1())
            .child(item("page-account", t!("settings.my_account"), Page::Account, cx))
            .child(item("page-profiles", t!("settings.profiles"), Page::Profiles, cx))
            .child(item("page-privacy", t!("settings.privacy"), Page::Privacy, cx))
            .child(item("page-devices", t!("settings.devices"), Page::Devices, cx))
            .child(item("page-accounts", t!("settings.accounts"), Page::Accounts, cx))
            .child(item("page-status", t!("settings.status"), Page::Status, cx))
            .child(separator())
            .child(widgets::label(t!("settings.app_settings")).px_2p5().pb_1())
            .child(item("page-appearance", t!("settings.appearance"), Page::Appearance, cx))
            .child(item("page-voice", t!("settings.voice"), Page::Voice, cx))
            .child(item("page-shortcuts", t!("settings.shortcuts"), Page::Shortcuts, cx))
            .child(item("page-language", t!("settings.language_time"), Page::Language, cx))
            .child(item("page-desktop", t!("settings.desktop"), Page::Desktop, cx))
            .child(separator())
            .child(
                div()
                    .id("settings-logout")
                    .px_2p5()
                    .py(zoomed(6.))
                    .rounded(theme::radius::control())
                    .font_weight(FontWeight::MEDIUM)
                    .cursor_pointer()
                    .text_color(rgb(theme::danger()))
                    .hover(|s| s.bg(theme::hover()))
                    .child(t!("settings.log_out"))
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(crate::Logout), cx)),
            );

        let page = match view.page {
            Page::Account => self.render_account(view, cx),
            Page::Profiles => self.render_profiles(view, cx),
            Page::Privacy => self.render_privacy(view, cx),
            Page::Devices => self.render_devices(view, cx),
            Page::Accounts => self.render_accounts(view, cx),
            Page::Status => self.render_status(view, cx),
            Page::Appearance => self.render_appearance(cx),
            Page::Voice => self.render_voice(view, cx),
            Page::Shortcuts => self.render_shortcuts(view, cx),
            Page::Language => self.render_language(view, cx),
            Page::Desktop => self.render_desktop(cx),
        };

        let close = div()
            .w(zoomed(72.))
            .flex_none()
            .pt(zoomed(60.))
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .child(
                div()
                    .id("close-settings")
                    .size(zoomed(36.))
                    .rounded_full()
                    .border_2()
                    .border_color(rgb(theme::interactive()))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(theme::interactive()))
                    .cursor_pointer()
                    .hover(|s| s.border_color(rgb(theme::text_strong())).text_color(rgb(theme::text_strong())))
                    .child("✕")
                    .on_click(cx.listener(|this, _, window, cx| this.close_overlay(window, cx))),
            )
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(theme::interactive()))
                    .child("ESC"),
            );

        div()
            .id("settings")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .bg(rgb(theme::base_lower()))
            .occlude()
            .child(sidebar)
            .child(
                div()
                    .id("settings-page")
                    .flex_1()
                    .min_w(zoomed(0.))
                    .h_full()
                    .overflow_y_scroll()
                    .pt(zoomed(60.))
                    .px_10()
                    .pb_10()
                    .child(div().max_w(zoomed(740.)).child(page)),
            )
            .child(close)
    }
}

pub(super) fn page_title(text: &'static str) -> gpui::Div {
    div()
        .mb_5()
        .text_size(zoomed(20.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(theme::text_strong()))
        .child(text)
}

pub(super) fn section_title(text: &'static str) -> gpui::Div {
    div()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(theme::text_strong()))
        .child(text)
}

/// The muted line under a setting's title.
pub(super) fn hint(text: impl Into<SharedString>) -> gpui::Div {
    div().text_size(zoomed(14.)).text_color(rgb(theme::text_muted())).child(text.into())
}

/// A setting with a switch on the right, as Discord lists them.
pub(super) fn switch_row(
    id: &'static str,
    title: &'static str,
    note: &'static str,
    on: bool,
    enabled: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_4()
        .py_3()
        .border_b_1()
        .border_color(theme::border())
        .when(!enabled, |row| row.opacity(0.5))
        .when(enabled, |row| row.cursor_pointer())
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w(zoomed(0.))
                .gap_1()
                .child(section_title(title).font_weight(FontWeight::MEDIUM))
                .child(hint(note)),
        )
        .child(widgets::toggle((id, 0usize), on))
}
