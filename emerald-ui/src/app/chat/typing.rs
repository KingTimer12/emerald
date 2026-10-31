//! The typing indicator over the bottom of the messages, just above the
//! message box: three pulsing dots, then who is typing.

use gpui::{AnyElement, Context, FontWeight, HighlightStyle, StyledText, div, prelude::*, rgb};

use super::Emerald;
use crate::theme;
use crate::zoom::zoomed;

/// One pulse of the three dots, as Discord's typing animation.
const PERIOD_MS: u128 = 1400;
/// How much later each dot pulses than the one before.
const STAGGER_MS: u128 = 200;
/// The part of a period a dot spends fading back after its peak.
const FADE: f32 = 0.4;
/// The dots change in steps of one stagger, so each dot peaks on a step.
/// The whole window redraws on each, so five a second keep a busy channel
/// cheap.
pub(in crate::app) const STEP_MS: u128 = STAGGER_MS;
// More than eight steps a second would make typing cost more than it shows.
const _: () = assert!(STEP_MS >= 125);

/// How lit `dot` (0 to 2) is `elapsed_ms` into the animation: fully at its
/// peak, fading to 0.3 and resting there until its next turn.
pub(super) fn dot_opacity(elapsed_ms: u128, dot: usize) -> f32 {
    let elapsed_ms = elapsed_ms - elapsed_ms % STEP_MS;
    let offset = STAGGER_MS * dot as u128;
    let phase = ((elapsed_ms + PERIOD_MS - offset % PERIOD_MS) % PERIOD_MS) as f32 / PERIOD_MS as f32;
    let pulse = (1. - phase / FADE).max(0.);
    0.3 + 0.7 * pulse
}

impl Emerald {
    /// The line over the bottom of the messages while someone types. The
    /// dots move only while the window is active and motion is allowed; the
    /// animation clock then asks for the next frame.
    pub(super) fn render_typing(&self) -> Option<AnyElement> {
        let text = self.typing_text()?;
        let animate = self.animates();
        if animate {
            self.typing_animating.set(true);
        }
        let elapsed = self.animation_clock.elapsed().as_millis();
        let dots = div().flex().items_center().gap(zoomed(2.)).children((0..3).map(|dot| {
            div()
                .size(zoomed(4.))
                .rounded_full()
                .bg(rgb(theme::text_strong()))
                .opacity(if animate { dot_opacity(elapsed, dot) } else { 1. })
        }));
        Some(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom_0()
                .h(zoomed(24.))
                .px_4()
                .flex()
                .items_center()
                .gap_2()
                .bg(rgb(theme::base_lower()))
                .text_xs()
                .text_color(rgb(theme::text_default()))
                .child(dots)
                .child(text)
                .into_any_element(),
        )
    }

    /// Redraws when the dots' next step is due; a step at a time, not on
    /// the 30 fps animation clock.
    pub(in crate::app) fn next_typing_frame(&mut self, cx: &mut Context<Self>) {
        if std::mem::replace(&mut self.typing_frame_pending, true) {
            return;
        }
        let elapsed = self.animation_clock.elapsed().as_millis();
        let wait = (STEP_MS - elapsed % STEP_MS) as u64;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(std::time::Duration::from_millis(wait)).await;
            this.update(cx, |this, cx| {
                this.typing_frame_pending = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// "Paulo is typing…", in the web client's wording.
    fn typing_text(&self) -> Option<StyledText> {
        let channel = self.selected_channel?;
        // Nicknames first, as the web client names typists in guilds.
        let guild = self.spaces.current().info();
        let names: Vec<&str> = self
            .typing
            .who(channel)
            .map(|user| {
                guild
                    .and_then(|g| g.member_style(user).0)
                    .or_else(|| self.people.display_name(user))
                    .unwrap_or(t!("common.someone"))
            })
            .collect();
        let (text, bold): (String, Vec<std::ops::Range<usize>>) = match names.as_slice() {
            [] => return None,
            [one] => t_spans!("chat.typing_one", name = one),
            [a, b] => t_spans!("chat.typing_two", a = a, b = b),
            _ => (t!("chat.typing_many").to_owned(), Vec::new()),
        };
        Some(StyledText::new(text).with_highlights(bold.into_iter().map(|range| {
            (
                range,
                HighlightStyle {
                    font_weight: Some(FontWeight::BOLD),
                    ..Default::default()
                },
            )
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dots_change_only_once_per_step() {
        assert_eq!(dot_opacity(STEP_MS, 0), dot_opacity(STEP_MS + STEP_MS - 1, 0));
        assert_ne!(dot_opacity(0, 0), dot_opacity(STEP_MS, 0));
    }

    #[test]
    fn dots_pulse_one_after_another() {
        assert!(dot_opacity(0, 0) > dot_opacity(0, 2));
        assert!(dot_opacity(400, 2) > dot_opacity(400, 0));
    }

    #[test]
    fn dots_stay_within_their_range() {
        for ms in (0..2800).step_by(50) {
            for dot in 0..3 {
                let opacity = dot_opacity(ms, dot);
                assert!((0.3..=1.0).contains(&opacity), "{ms} {dot} {opacity}");
            }
        }
    }
}
