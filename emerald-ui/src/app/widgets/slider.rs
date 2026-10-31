//! Discord's volume slider: a thin track filled in the brand color up to a
//! round knob. Clicking the track jumps there; dragging follows the pointer
//! even outside the track, clamped to its ends.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    App, Bounds, ElementId, Empty, IntoElement, MouseButton, Pixels, Render, Window, canvas, div, prelude::*,
    rgb,
};

use crate::theme;
use crate::zoom::zoomed;

/// The track's thickness and the knob's size, as on discord.com.
const TRACK: f32 = 4.;
const KNOB: f32 = 16.;

/// The value under the pointer at `x`, for a track starting at `left` and
/// `width` wide, clamped to `0..=max`.
pub fn value_at(x: f32, left: f32, width: f32, max: f32) -> f32 {
    if width <= 0. {
        return 0.;
    }
    ((x - left) / width).clamp(0., 1.) * max
}

/// What a slider drag carries, so each slider follows only its own drag.
struct Dragging(ElementId);

/// The drag shows nothing under the pointer; the knob itself moves.
struct NoPreview;

impl Render for NoPreview {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        Empty
    }
}

/// A slider from 0 to `max` showing `value`. `on_change` gets the new value
/// on a click on the track and on every move of a drag; it takes it by
/// reference so `Context::listener` fits.
pub fn slider(
    id: impl Into<ElementId>,
    value: f32,
    max: f32,
    on_change: impl Fn(&f32, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let id = id.into();
    let fraction = if max > 0. { (value / max).clamp(0., 1.) } else { 0. };
    // Painted bounds of the track, read by the click handler.
    let track: Rc<Cell<Bounds<Pixels>>> = Rc::default();
    let on_change = Rc::new(on_change);
    let pick = {
        let (track, on_change) = (track.clone(), on_change.clone());
        move |x: Pixels, window: &mut Window, cx: &mut App| {
            let bounds = track.get();
            on_change(&value_at(x.into(), bounds.left().into(), bounds.size.width.into(), max), window, cx);
        }
    };
    let drag_pick = pick.clone();
    let drag_id = id.clone();
    div()
        .id(id.clone())
        .relative()
        .w_full()
        .h(zoomed(KNOB))
        .flex()
        .items_center()
        .cursor_pointer()
        .child({
            let track = track.clone();
            canvas(move |bounds, _, _| track.set(bounds), |_, _, _, _| {}).absolute().size_full()
        })
        .child(
            div()
                .w_full()
                .h(zoomed(TRACK))
                .rounded_full()
                .bg(rgb(theme::interactive_muted()))
                .child(div().h_full().w(gpui::relative(fraction)).rounded_full().bg(rgb(theme::brand()))),
        )
        .child(
            div()
                .absolute()
                .left(gpui::relative(fraction))
                .ml(zoomed(-KNOB / 2.))
                .size(zoomed(KNOB))
                .rounded_full()
                .bg(rgb(theme::on_color()))
                .border_1()
                .border_color(theme::border_normal())
                .shadow_sm(),
        )
        .on_mouse_down(MouseButton::Left, move |event, window, cx| pick(event.position.x, window, cx))
        .on_drag(Dragging(id), |_, _, _, cx| cx.new(|_| NoPreview))
        .on_drag_move::<Dragging>(move |event, window, cx| {
            if event.drag(cx).0 == drag_id {
                drag_pick(event.event.position.x, window, cx);
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_follows_the_pointer_inside_the_track() {
        assert_eq!(value_at(150., 100., 200., 200.), 50.);
    }

    #[test]
    fn value_clamps_past_either_end() {
        assert_eq!(value_at(50., 100., 200., 200.), 0.);
        assert_eq!(value_at(900., 100., 200., 200.), 200.);
    }

    #[test]
    fn a_zero_width_track_reads_as_zero() {
        assert_eq!(value_at(100., 100., 0., 200.), 0.);
    }
}
