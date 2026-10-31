//! The call panel over the user panel ("Voice Connected", the channel,
//! hang up), and the mute and deafen buttons in the user panel.

use super::audio_menu::AudioMenuKind;
use gpui::{AnyElement, Context, FontWeight, SharedString, div, prelude::*, rgb};

use super::super::super::{icons, theme};
use super::super::{Emerald, widgets};
use emerald_discord::CallStatus;
use crate::zoom::zoomed;

/// Height of the user panel plus its margins.
const USER_PANEL: f32 = 72.;
/// Height of the call panel over it.
const CALL_PANEL: f32 = 56.;

impl Emerald {
    /// How much the bottom of the sidebars stays clear of the floating panels.
    pub(in crate::app) fn panel_clearance(&self) -> f32 {
        if self.call.is_some() || self.call_notice.is_some() { USER_PANEL + CALL_PANEL } else { USER_PANEL }
    }

    pub(in crate::app) fn render_call_panel(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let panel = div()
            .absolute()
            .left_2()
            .bottom(zoomed(USER_PANEL))
            .w(zoomed(super::super::sidebar::SERVER_LIST_WIDTH + super::super::sidebar::SIDEBAR_WIDTH - 16.))
            .h(zoomed(CALL_PANEL - 8.))
            .px_2()
            .flex()
            .items_center()
            .gap_1()
            .rounded(theme::radius::control())
            .border_1()
            .border_color(theme::border())
            .bg(rgb(theme::base_low()));
        let Some(call) = &self.call else {
            let notice = self.call_notice.clone()?;
            return Some(
                panel
                    .child(
                        div()
                            .flex_1()
                            .min_w(zoomed(0.))
                            .px_1()
                            .text_size(zoomed(13.))
                            .text_color(rgb(theme::danger()))
                            .child(notice),
                    )
                    .child(widgets::svg_button("voice-notice-close", icons::CLOSE).on_click(cx.listener(|this, _, _, cx| {
                        this.call_notice = None;
                        cx.notify();
                    })))
                    .into_any_element(),
            );
        };
        let (color, encrypted) = match &call.status {
            CallStatus::Connected { privacy_code } => (theme::online(), privacy_code.clone()),
            _ => (theme::idle(), None),
        };
        let space = call.guild.and_then(|guild| self.spaces.iter().find(|s| s.id == guild));
        let channel = space
            .and_then(|s| s.channels().iter().find(|c| c.id == call.channel))
            .and_then(|c| c.name.clone())
            .unwrap_or_default();
        let place: SharedString = match space {
            Some(space) => format!("{channel} / {}", space.name).into(),
            // A DM call: the DM's name, as in the list.
            None => self.spaces.dms().label(call.channel).cloned().unwrap_or_default(),
        };
        let status = if call.suppressed && call.connected() {
            t!("voice.audience").into()
        } else if call.listen_only && call.connected() {
            t!("voice.listen_only").into()
        } else {
            Self::call_status_text(&call.status)
        };
        Some(
            panel
                .child(
                    div()
                        .flex_1()
                        .min_w(zoomed(0.))
                        .px_1()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .text_size(zoomed(14.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(color))
                                .child(div().truncate().child(status))
                                .when_some(encrypted, |row, code| {
                                    row.child(
                                        div()
                                            .id("voice-encrypted")
                                            .child(icons::icon(icons::SHIELD, 14., theme::online()))
                                            .tooltip(widgets::tooltip(t!("voice.encrypted", code = code))),
                                    )
                                }),
                        )
                        .child(div().truncate().text_xs().text_color(rgb(theme::text_muted())).child(place)),
                )
                .child(
                    widgets::svg_button("voice-hang-up", icons::HANG_UP)
                        .tooltip(widgets::tooltip(t!("voice.disconnect")))
                        .on_click(cx.listener(|this, _, _, cx| this.leave_voice(cx))),
                )
                .into_any_element(),
        )
    }

    /// Mute and deafen, red when on, each with the chevron of its device
    /// menu, as beside the web client's name.
    pub(in crate::app) fn render_voice_buttons(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let voice = &self.settings.voice;
        let muted = voice.muted || voice.deafened;
        let open = |kind| self.audio_menu.as_ref().is_some_and(|menu| menu.kind == kind);
        // The menu opens over the whole split button, wherever on the
        // chevron the click landed.
        let input = widgets::popover::Tracked::default();
        let output = widgets::popover::Tracked::default();
        let chevron = |id, kind: AudioMenuKind, tip, tracked: &widgets::popover::Tracked, cx: &mut Context<Self>| {
            let tracked = tracked.clone();
            widgets::chevron_button(id, open(kind))
                .tooltip(widgets::tooltip(tip))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.open_audio_menu(kind, widgets::popover::Anchor::above(tracked.bounds()), cx)
                }))
        };
        div()
            .flex()
            .child(input.track(widgets::split_button(
                toggle("voice-mute", if muted { icons::MICROPHONE_OFF } else { icons::MICROPHONE }, muted)
                    .tooltip(widgets::tooltip(if muted { t!("voice.unmute") } else { t!("voice.mute") }))
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_mute(cx))),
                chevron("voice-input-menu", AudioMenuKind::Input, t!("voice.input_options"), &input, cx),
            )))
            .child(output.track(widgets::split_button(
                toggle(
                    "voice-deafen",
                    if voice.deafened { icons::HEADPHONES_OFF } else { icons::HEADPHONES },
                    voice.deafened,
                )
                .tooltip(widgets::tooltip(if voice.deafened { t!("voice.undeafen") } else { t!("voice.deafen") }))
                .on_click(cx.listener(|this, _, _, cx| this.toggle_deafen(cx))),
                chevron("voice-output-menu", AudioMenuKind::Output, t!("voice.output_options"), &output, cx),
            )))
    }
}

/// An icon button drawn in the danger color while `on`.
fn toggle(id: &'static str, path: &'static str, on: bool) -> gpui::Stateful<gpui::Div> {
    let color = if on { theme::danger() } else { theme::interactive() };
    div()
        .id(id)
        .group(id)
        .flex_none()
        .size(zoomed(32.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme::radius::control())
        .cursor_pointer()
        .hover(|s| s.bg(theme::hover()))
        .child(icons::icon(path, 20., color).when(!on, |icon| {
            icon.group_hover(id, |s| s.text_color(rgb(theme::text_strong())))
        }))
}
