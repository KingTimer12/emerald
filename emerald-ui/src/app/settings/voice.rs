//! Voice: which microphone and speakers, how loud, how sensitive, and
//! noise suppression. Changes reach a call in progress at once.

use super::*;
use crate::settings::VoiceSettings;

/// Volumes offered, in percent.
const VOLUMES: [u16; 6] = [50, 75, 100, 125, 150, 200];
/// Sensitivities offered, in decibels: lower sends quieter sound.
const SENSITIVITIES: [i16; 5] = [-70, -60, -50, -40, -30];

impl Emerald {
    /// Lists the devices off the UI thread; slow on some systems.
    pub(in crate::app) fn list_audio_devices(&mut self, cx: &mut Context<Self>) {
        let task = cx.background_executor().spawn(async { emerald_audio::devices() });
        cx.spawn(async move |this, cx| {
            let devices = task.await;
            this.update(cx, |this, cx| {
                this.audio_devices = Some(devices);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn change_voice(&mut self, change: impl FnOnce(&mut VoiceSettings), cx: &mut Context<Self>) {
        change(&mut self.settings.voice);
        self.voice_controls_changed(cx);
    }

    pub(super) fn render_voice(&self, _view: &SettingsView, cx: &mut Context<Self>) -> AnyElement {
        let voice = &self.settings.voice;
        let devices = |id: &'static str, list: Option<&[emerald_audio::Device]>, chosen: Option<&String>, input: bool, cx: &mut Context<Self>| {
            let Some(list) = list else {
                return div().child(hint(t!("settings.voice_listing")));
            };
            let default = widgets::chip((id, 0), t!("settings.voice_default_device"), chosen.is_none()).on_click(
                cx.listener(move |this, _, _, cx| {
                    this.change_voice(|v| if input { v.input = None } else { v.output = None }, cx)
                }),
            );
            div().flex().flex_wrap().gap_2().child(default).children(list.iter().enumerate().map(|(ix, device)| {
                let device_id = device.id.clone();
                widgets::chip((id, ix + 1), device.name.clone(), chosen == Some(&device.id)).on_click(cx.listener(
                    move |this, _, _, cx| {
                        let picked = Some(device_id.clone());
                        this.change_voice(|v| if input { v.input = picked } else { v.output = picked }, cx)
                    },
                ))
            }))
        };
        let listed = self.audio_devices.as_ref();
        let volumes = |id: &'static str, current: u16, input: bool, cx: &mut Context<Self>| {
            div().flex().gap_2().children(VOLUMES.into_iter().enumerate().map(|(ix, level)| {
                widgets::chip((id, ix), format!("{level}%"), level == current).on_click(cx.listener(move |this, _, _, cx| {
                    this.change_voice(|v| if input { v.input_volume = level } else { v.output_volume = level }, cx)
                }))
            }))
        };

        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(page_title(t!("settings.voice")))
            .child(section_title(t!("settings.voice_input")).mt_2())
            .child(devices("voice-input", listed.map(|d| d.inputs.as_slice()), voice.input.as_ref(), true, cx))
            .child(section_title(t!("settings.voice_output")).mt_6())
            .child(devices("voice-output", listed.map(|d| d.outputs.as_slice()), voice.output.as_ref(), false, cx))
            .child(section_title(t!("settings.voice_input_volume")).mt_6())
            .child(volumes("voice-input-volume", voice.input_volume, true, cx))
            .child(section_title(t!("settings.voice_output_volume")).mt_6())
            .child(volumes("voice-output-volume", voice.output_volume, false, cx))
            .child(section_title(t!("settings.voice_sensitivity")).mt_6())
            .child(hint(t!("settings.voice_sensitivity_hint")))
            .child(div().flex().gap_2().children(SENSITIVITIES.into_iter().enumerate().map(|(ix, level)| {
                widgets::chip(("voice-sensitivity", ix), format!("{level} dB"), level == voice.sensitivity).on_click(
                    cx.listener(move |this, _, _, cx| this.change_voice(|v| v.sensitivity = level, cx)),
                )
            })))
            .child(
                div()
                    .id("noise-suppression")
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
                            .child(section_title(t!("settings.voice_noise")))
                            .child(hint(t!("settings.voice_noise_hint"))),
                    )
                    .child(widgets::toggle("noise-toggle", voice.noise_suppression))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.change_voice(|v| v.noise_suppression = !v.noise_suppression, cx)
                    })),
            )
            .into_any_element()
    }
}
