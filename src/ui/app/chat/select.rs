//! Selecting a message's text by dragging over it, as in a browser: a
//! double click takes a word, a triple click the whole text, and the usual
//! shortcut copies it. One message at a time.

use std::ops::Range;

use gpui::{
    AnyElement, ClipboardItem, Context, CursorStyle, DispatchPhase, HitboxBehavior, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, SharedString, TextLayout, WeakEntity, canvas, prelude::*,
};

use super::super::Emerald;
use crate::discord::model::Snowflake;

/// The selected part of one message's text.
pub struct TextSelection {
    message: Snowflake,
    /// The text as drawn, mentions resolved: what gets copied.
    text: SharedString,
    anchor: usize,
    head: usize,
    /// The left button is still down since the selection started.
    dragging: bool,
}

impl TextSelection {
    /// The selected byte range of `message`'s text, if that is the message
    /// and its text is still `text`.
    pub fn range_in(&self, message: Snowflake, text: &str) -> Option<Range<usize>> {
        let range = self.anchor.min(self.head)..self.anchor.max(self.head);
        (self.message == message && self.text == text && !range.is_empty()).then_some(range)
    }

    fn selected(&self) -> Option<&str> {
        let range = self.anchor.min(self.head)..self.anchor.max(self.head);
        self.text.get(range).filter(|s| !s.is_empty())
    }
}

/// What a message's text needs to be selectable.
pub struct Selectable {
    pub message: Snowflake,
    pub view: WeakEntity<Emerald>,
}

impl Emerald {
    /// Copies the selected message text; `false` when nothing is selected
    /// in the open channel.
    pub(in super::super) fn copy_selection(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(selection) = &self.text_selection else { return false };
        let shown = self
            .selected_channel
            .is_some_and(|c| self.history.find(c, selection.message).is_some());
        match selection.selected().filter(|_| shown) {
            Some(text) => {
                cx.write_to_clipboard(ClipboardItem::new_string(text.to_owned()));
                true
            }
            None => false,
        }
    }
}

/// An invisible layer under a message's text that turns presses and drags
/// over it into a selection. Painted before the text, so links keep their
/// pointer cursor on top of the text cursor.
pub(super) fn selection_layer(selectable: Selectable, text: SharedString, layout: TextLayout) -> AnyElement {
    let Selectable { message, view } = selectable;
    canvas(
        |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
        move |_, hitbox, window, _| {
            window.set_cursor_style(CursorStyle::IBeam, &hitbox);
            let index = {
                let layout = layout.clone();
                move |position| layout.index_for_position(position).unwrap_or_else(|ix| ix)
            };

            let (view_down, text_down, index_down) = (view.clone(), text.clone(), index.clone());
            window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble || event.button != MouseButton::Left {
                    return;
                }
                let inside = hitbox.is_hovered(window);
                view_down
                    .update(cx, |this, cx| {
                        if inside {
                            let ix = index_down(event.position);
                            let (anchor, head) = match event.click_count {
                                1 => (ix, ix),
                                2 => word_at(&text_down, ix),
                                _ => (0, text_down.len()),
                            };
                            this.text_selection = Some(TextSelection {
                                message,
                                text: text_down.clone(),
                                anchor,
                                head,
                                dragging: event.click_count == 1,
                            });
                            cx.notify();
                        } else if this.text_selection.as_ref().is_some_and(|s| s.message == message) {
                            // A press anywhere else drops this selection.
                            this.text_selection = None;
                            cx.notify();
                        }
                    })
                    .ok();
            });

            let view_move = view.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                if phase != DispatchPhase::Bubble {
                    return;
                }
                view_move
                    .update(cx, |this, cx| {
                        let Some(selection) = this
                            .text_selection
                            .as_mut()
                            .filter(|s| s.message == message && s.dragging)
                        else {
                            return;
                        };
                        // The release can land outside the window.
                        if event.pressed_button != Some(MouseButton::Left) {
                            selection.dragging = false;
                            return;
                        }
                        let ix = clamp(&selection.text, index(event.position));
                        if selection.head != ix {
                            selection.head = ix;
                            cx.notify();
                        }
                    })
                    .ok();
            });

            let view_up = view.clone();
            window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                if phase != DispatchPhase::Bubble || event.button != MouseButton::Left {
                    return;
                }
                view_up
                    .update(cx, |this, _| {
                        if let Some(selection) = this.text_selection.as_mut().filter(|s| s.message == message) {
                            selection.dragging = false;
                        }
                    })
                    .ok();
            });
        },
    )
    .absolute()
    .size_full()
    .into_any_element()
}

/// Keeps an index from a drag on a char boundary of `text`.
fn clamp(text: &str, mut ix: usize) -> usize {
    ix = ix.min(text.len());
    while !text.is_char_boundary(ix) {
        ix -= 1;
    }
    ix
}

/// The word around `ix`: a run of letters and digits, or of anything else
/// that is not whitespace, as a double click picks in a browser.
fn word_at(text: &str, ix: usize) -> (usize, usize) {
    let ix = clamp(text, ix);
    let kind = |c: char| {
        if c.is_alphanumeric() || c == '_' {
            1
        } else if c.is_whitespace() {
            0
        } else {
            2
        }
    };
    let Some(here) = text[ix..].chars().next().or_else(|| text[..ix].chars().next_back()) else {
        return (ix, ix);
    };
    let class = kind(here);
    let start = text[..ix]
        .char_indices()
        .rev()
        .take_while(|(_, c)| kind(*c) == class)
        .last()
        .map_or(ix, |(i, _)| i);
    let end = text[ix..]
        .char_indices()
        .find(|(_, c)| kind(*c) != class)
        .map_or(text.len(), |(i, _)| ix + i);
    (start, end)
}

#[cfg(test)]
mod tests {
    use super::word_at;

    #[test]
    fn double_click_takes_the_word() {
        let text = "olá mundo, tudo bem?";
        assert_eq!(&text[word_at(text, 1).0..word_at(text, 1).1], "olá");
        assert_eq!(&text[word_at(text, 7).0..word_at(text, 7).1], "mundo");
        let (start, end) = word_at(text, text.len());
        assert_eq!(&text[start..end], "?");
    }
}
