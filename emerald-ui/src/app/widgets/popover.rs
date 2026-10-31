//! The floating panel menus and popovers share: rounded, bordered, raised,
//! and opaque to the mouse so clicks inside never reach what is under it.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    AnyElement, Bounds, Corner, Div, ElementId, Pixels, Point, Stateful, anchored, canvas, deferred, div, point,
    prelude::*, rgb,
};

use crate::theme;
use crate::zoom::zoomed;

/// The space between a button and the popover it opened.
const GAP: f32 = 8.;

/// Where a popover opens: at the pointer for a context menu, or against the
/// button that opened it, so it does not move with where the click landed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    /// The popover's corner placed at `position`.
    corner: Corner,
    position: Point<Pixels>,
}

impl Anchor {
    /// A context menu's: its top-left corner at the pointer.
    pub fn pointer(position: Point<Pixels>) -> Self {
        Self { corner: Corner::TopLeft, position }
    }

    /// Under `button`, its left edges lined up.
    pub fn below(button: Bounds<Pixels>) -> Self {
        Self::below_at(button, zoomed(GAP))
    }

    fn below_at(button: Bounds<Pixels>, gap: Pixels) -> Self {
        Self { corner: Corner::TopLeft, position: button.bottom_left() + point(Pixels::ZERO, gap) }
    }

    /// Under `button`, its right edges lined up, for buttons at the right.
    pub fn below_end(button: Bounds<Pixels>) -> Self {
        Self::below_end_at(button, zoomed(GAP))
    }

    fn below_end_at(button: Bounds<Pixels>, gap: Pixels) -> Self {
        Self { corner: Corner::TopRight, position: button.bottom_right() + point(Pixels::ZERO, gap) }
    }

    /// Over `button`, its left edges lined up, for buttons at the bottom.
    pub fn above(button: Bounds<Pixels>) -> Self {
        Self::above_at(button, zoomed(GAP))
    }

    fn above_at(button: Bounds<Pixels>, gap: Pixels) -> Self {
        Self { corner: Corner::BottomLeft, position: button.origin - point(Pixels::ZERO, gap) }
    }

    /// Over `button`, its right edges lined up.
    pub fn above_end(button: Bounds<Pixels>) -> Self {
        Self::above_end_at(button, zoomed(GAP))
    }

    fn above_end_at(button: Bounds<Pixels>, gap: Pixels) -> Self {
        Self { corner: Corner::BottomRight, position: button.top_right() - point(Pixels::ZERO, gap) }
    }

    /// Left of `button`, their tops lined up, as the message toolbar's
    /// menus open.
    pub fn left_of(button: Bounds<Pixels>) -> Self {
        Self::left_of_at(button, zoomed(GAP))
    }

    fn left_of_at(button: Bounds<Pixels>, gap: Pixels) -> Self {
        Self { corner: Corner::TopRight, position: button.origin - point(gap, Pixels::ZERO) }
    }
}

/// A press outside a popover closes it before the click that follows
/// lands; a click on its own button this soon after is that press.
pub const CLOSE_GRACE: std::time::Duration = std::time::Duration::from_millis(500);

/// Whether a popover that closed at `closed` (by a press outside it) did so
/// just now, so a click on its button at `now` must not reopen it.
pub fn just_closed(closed: Option<std::time::Instant>, now: std::time::Instant) -> bool {
    closed.is_some_and(|at| now.saturating_duration_since(at) < CLOSE_GRACE)
}

/// The bounds an element was last drawn at, read when it is clicked so a
/// popover can open against it. Cheap to clone; clones share the bounds.
#[derive(Clone, Default)]
pub struct Tracked(Rc<Cell<Bounds<Pixels>>>);

impl Tracked {
    pub fn bounds(&self) -> Bounds<Pixels> {
        self.0.get()
    }

    /// `element`, recording its bounds each time it is drawn. Layout places
    /// an absolute child against its parent, positioned or not, so the
    /// element keeps its own position.
    pub fn track<E: ParentElement + Styled>(&self, element: E) -> E {
        let cell = self.0.clone();
        element.child(
            canvas(move |bounds, _, _| cell.set(bounds), |_, _, _, _| {}).absolute().top_0().left_0().size_full(),
        )
    }
}

/// The panel itself, `width` wide; the caller adds items and dismissal.
pub fn panel(id: impl Into<ElementId>, width: f32) -> Stateful<Div> {
    div()
        .id(id)
        .w(zoomed(width))
        .flex()
        .flex_col()
        .p_2()
        .rounded(theme::radius::modal())
        .border_1()
        .border_color(theme::border_normal())
        .bg(rgb(theme::surface_high()))
        .shadow_lg()
        .occlude()
}

/// `child` drawn over everything at `anchor`, kept inside the window.
pub fn at(anchor: Anchor, child: impl IntoElement) -> AnyElement {
    at_priority(anchor, 1, child)
}

/// [`at`], over other popovers drawn at a lower `priority`.
pub fn at_priority(anchor: Anchor, priority: usize, child: impl IntoElement) -> AnyElement {
    deferred(
        anchored()
            .anchor(anchor.corner)
            .position(anchor.position)
            .snap_to_window_with_margin(zoomed(8.))
            .child(child),
    )
    .with_priority(priority)
    .into_any_element()
}

/// A thin line between groups of items.
pub fn separator() -> Div {
    div().my_1().h(zoomed(1.)).bg(theme::border_normal())
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{px, size};

    fn button() -> Bounds<Pixels> {
        Bounds { origin: point(px(100.), px(200.)), size: size(px(32.), px(20.)) }
    }

    #[test]
    fn a_click_right_after_a_close_does_not_reopen() {
        let now = std::time::Instant::now();
        assert!(just_closed(Some(now), now));
        assert!(!just_closed(Some(now), now + CLOSE_GRACE));
        assert!(!just_closed(None, now));
    }

    #[test]
    fn the_place_does_not_depend_on_where_the_click_landed() {
        // Only the button's bounds go in, so two clicks on it open alike.
        assert_eq!(Anchor::below_at(button(), px(8.)), Anchor::below_at(button(), px(8.)));
    }

    #[test]
    fn each_side_puts_the_right_corner_beside_the_button() {
        let gap = px(8.);
        assert_eq!(
            Anchor::below_at(button(), gap),
            Anchor { corner: Corner::TopLeft, position: point(px(100.), px(228.)) }
        );
        assert_eq!(
            Anchor::below_end_at(button(), gap),
            Anchor { corner: Corner::TopRight, position: point(px(132.), px(228.)) }
        );
        assert_eq!(
            Anchor::above_at(button(), gap),
            Anchor { corner: Corner::BottomLeft, position: point(px(100.), px(192.)) }
        );
        assert_eq!(
            Anchor::above_end_at(button(), gap),
            Anchor { corner: Corner::BottomRight, position: point(px(132.), px(192.)) }
        );
        assert_eq!(
            Anchor::left_of_at(button(), gap),
            Anchor { corner: Corner::TopRight, position: point(px(92.), px(200.)) }
        );
    }
}
