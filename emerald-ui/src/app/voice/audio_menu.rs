//! The menus behind the chevrons beside the microphone and headset buttons:
//! which device, how loud, and a way to the voice settings.

use gpui::{AnyElement, Context, SharedString, Window, div, prelude::*};

use emerald_audio::Device;

use super::super::place_menu::{item, item_two_line};
use super::super::settings::Page;
use super::super::widgets::popover::{self, Anchor};
use super::super::widgets::slider;
use super::super::Emerald;
use crate::icons;
use crate::zoom::zoomed;

/// Volumes go up to twice the device's own, as on the Voice settings page.
const MAX_VOLUME: f32 = 200.;
const WIDTH: f32 = 220.;

/// Which button's menu is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum AudioMenuKind {
    Input,
    Output,
}

/// The open menu, where it opened, and whether it lists the devices.
pub(in crate::app) struct AudioMenu {
    pub kind: AudioMenuKind,
    anchor: Anchor,
    choosing_device: bool,
}

/// The chosen device's name; the system default when none is chosen, the
/// list is not there yet, or the chosen device is gone.
pub(in crate::app) fn device_label(devices: Option<&[Device]>, chosen: Option<&str>) -> SharedString {
    devices
        .zip(chosen)
        .and_then(|(devices, chosen)| devices.iter().find(|device| device.id == chosen))
        .map_or_else(|| t!("settings.voice_default_device").into(), |device| device.name.clone().into())
}

/// Whether a click on `kind`'s chevron at `now` opens its menu, given the
/// menu last closed by a press outside it.
pub(in crate::app) fn opens(
    dismissed: Option<(AudioMenuKind, std::time::Instant)>,
    kind: AudioMenuKind,
    now: std::time::Instant,
) -> bool {
    !dismissed.is_some_and(|(closed, at)| closed == kind && popover::just_closed(Some(at), now))
}

/// Chooses `device` (`None`: the system default) for the input or output.
fn pick_device(
    input: bool,
    device: Option<String>,
    cx: &mut Context<Emerald>,
) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static {
    cx.listener(move |this, _, _, cx| {
        let device = device.clone();
        if input {
            this.settings.voice.input = device;
        } else {
            this.settings.voice.output = device;
        }
        this.audio_menu = None;
        this.voice_controls_changed(cx);
    })
}

impl Emerald {
    /// Opens `kind`'s menu at `anchor`, or closes it when it is open.
    /// The device list is read again each time, off the UI thread.
    pub(in crate::app) fn open_audio_menu(&mut self, kind: AudioMenuKind, anchor: Anchor, cx: &mut Context<Self>) {
        let dismissed = self.audio_menu_dismissed.take();
        if self.audio_menu.as_ref().is_some_and(|menu| menu.kind == kind) {
            self.audio_menu = None;
        } else if opens(dismissed, kind, std::time::Instant::now()) {
            self.audio_menu = Some(AudioMenu { kind, anchor, choosing_device: false });
            self.list_audio_devices(cx);
        }
        cx.notify();
    }

    pub(in crate::app) fn render_audio_menu(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let menu = self.audio_menu.as_ref()?;
        let input = menu.kind == AudioMenuKind::Input;
        let voice = &self.settings.voice;
        let devices = self
            .audio_devices
            .as_ref()
            .map(|all| if input { all.inputs.as_slice() } else { all.outputs.as_slice() });
        let chosen = if input { voice.input.as_deref() } else { voice.output.as_deref() };
        let items: Vec<AnyElement> = if menu.choosing_device {
            let mut items = vec![
                item(("audio-device", 0), t!("settings.voice_default_device").into(), chosen.is_none().then_some(icons::CHECK))
                    .on_click(pick_device(input, None, cx))
                    .into_any_element(),
            ];
            items.extend(devices.unwrap_or_default().iter().enumerate().map(|(ix, device)| {
                let on = chosen == Some(device.id.as_str());
                item(("audio-device", ix + 1), device.name.clone().into(), on.then_some(icons::CHECK))
                    .on_click(pick_device(input, Some(device.id.clone()), cx))
                    .into_any_element()
            }));
            items
        } else {
            let (title, volume_title, volume) = if input {
                (t!("voice.input_device"), t!("voice.input_volume"), voice.input_volume)
            } else {
                (t!("voice.output_device"), t!("voice.output_volume"), voice.output_volume)
            };
            let volume_slider = slider::slider(
                if input { "input-volume" } else { "output-volume" },
                volume as f32,
                MAX_VOLUME,
                cx.listener(move |this: &mut Self, value: &f32, _, cx| {
                    let value = value.round() as u16;
                    let voice = &mut this.settings.voice;
                    let current = if input { &mut voice.input_volume } else { &mut voice.output_volume };
                    if *current != value {
                        *current = value;
                        this.voice_volume_changed(cx);
                    }
                }),
            );
            vec![
                item_two_line(("audio-menu", 0), title.into(), device_label(devices, chosen), Some(icons::CHEVRON_RIGHT))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(menu) = &mut this.audio_menu {
                            menu.choosing_device = true;
                        }
                        cx.notify();
                    }))
                    .into_any_element(),
                popover::separator().into_any_element(),
                div()
                    .px_2()
                    .py_1()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().text_size(zoomed(14.)).child(volume_title))
                    .child(volume_slider)
                    .into_any_element(),
                popover::separator().into_any_element(),
                item(("audio-menu", 1), t!("voice.settings").into(), Some(icons::SETTINGS))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.audio_menu = None;
                        this.open_settings(Page::Voice, window, cx);
                    }))
                    .into_any_element(),
            ]
        };
        let panel = popover::panel("audio-menu", WIDTH)
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.audio_menu_dismissed = this.audio_menu.take().map(|menu| (menu.kind, std::time::Instant::now()));
                cx.notify();
            }))
            .children(items);
        Some(popover::at(menu.anchor, panel))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_click_on_the_chevron_that_just_closed_its_menu_does_not_reopen_it() {
        let now = std::time::Instant::now();
        assert!(!opens(Some((AudioMenuKind::Input, now)), AudioMenuKind::Input, now));
    }

    #[test]
    fn the_other_chevron_or_a_later_click_opens() {
        let then = std::time::Instant::now();
        assert!(opens(Some((AudioMenuKind::Input, then)), AudioMenuKind::Output, then));
        assert!(opens(Some((AudioMenuKind::Input, then)), AudioMenuKind::Input, then + std::time::Duration::from_secs(2)));
        assert!(opens(None, AudioMenuKind::Input, then));
    }

    #[test]
    fn device_label_names_the_chosen_device() {
        let devices = [Device { id: "a".into(), name: "Realtek(R) Audio".into() }];
        assert_eq!(device_label(Some(&devices), Some("a")).as_ref(), "Realtek(R) Audio");
    }

    #[test]
    fn device_label_falls_back_to_default() {
        assert_eq!(device_label(None, Some("a")).as_ref(), t!("settings.voice_default_device"));
        assert_eq!(device_label(Some(&[]), None).as_ref(), t!("settings.voice_default_device"));
    }
}
