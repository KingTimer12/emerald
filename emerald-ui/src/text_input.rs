//! Text input, adapted from GPUI's `examples/input.rs` (IME, selection,
//! clipboard) with a submit action and a dark theme. Single-line by
//! default; the message box is multi-line: Shift+Enter breaks the line,
//! long lines wrap, and the box grows up to a limit, then scrolls.
//!
//! [`editing`] handles keys and the mouse, [`offsets`] converts between
//! byte, UTF-16 and grapheme offsets, [`layout`] shapes the text,
//! [`element`] paints it and [`ime`] talks to the platform's input methods.

use crate::zoom::zoomed;

mod editing;
mod element;
mod ime;
mod layout;
mod offsets;

use element::*;
use layout::*;
use offsets::*;

use std::ops::Range;

use gpui::{
    App, AvailableSpace, Bounds, ClipboardItem, ContentMask, Context, CursorStyle, ElementId, ElementInputHandler,
    Entity, EntityInputHandler, EventEmitter, FocusHandle, Focusable, GlobalElementId, KeyBinding, LayoutId,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, ScrollWheelEvent,
    SharedString, Style, TextAlign, TextRun, UTF16Selection, UnderlineStyle, Window, WrappedLine, actions, div, fill,
    point, prelude::*, relative, rgb, rgba, size,
};
use unicode_segmentation::*;

use super::theme;

actions!(
    text_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        Up,
        Down,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectAll,
        Home,
        End,
        ShowCharacterPalette,
        Paste,
        Cut,
        Copy,
        Submit,
        Newline,
        Tab,
    ]
);

pub fn bind_keys(cx: &mut App) {
    let context = Some("TextInput");
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, context),
        KeyBinding::new("delete", Delete, context),
        KeyBinding::new("left", Left, context),
        KeyBinding::new("right", Right, context),
        KeyBinding::new("up", Up, context),
        KeyBinding::new("down", Down, context),
        KeyBinding::new("shift-left", SelectLeft, context),
        KeyBinding::new("shift-right", SelectRight, context),
        KeyBinding::new("shift-up", SelectUp, context),
        KeyBinding::new("shift-down", SelectDown, context),
        KeyBinding::new("secondary-a", SelectAll, context),
        KeyBinding::new("secondary-v", Paste, context),
        KeyBinding::new("secondary-c", Copy, context),
        KeyBinding::new("secondary-x", Cut, context),
        KeyBinding::new("home", Home, context),
        KeyBinding::new("end", End, context),
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, context),
        KeyBinding::new("enter", Submit, context),
        KeyBinding::new("shift-enter", Newline, context),
        KeyBinding::new("tab", Tab, context),
    ]);
}

/// Emitted with the trimmed content when the user presses enter.
pub struct Submitted(pub String);

/// An image pasted while the input had focus; the owner may attach it.
pub struct PastedImage(pub gpui::Image);

/// A key meant for the menu the owner shows over the input, such as the
/// autocomplete list, while [`TextInput::set_menu_open`] is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuKey {
    Up,
    Down,
    /// Enter or Tab: take the highlighted entry.
    Accept,
}

pub struct TextInput {
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Option<Layout>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
    /// Chat input clears on enter; form fields keep their text so a failed
    /// submit can be corrected.
    clear_on_submit: bool,
    /// Small single-line field, like the sidebar search.
    compact: bool,
    /// Enter on an empty field still submits: the message box can send
    /// attachments alone.
    submit_empty: bool,
    /// Rows shown before the text scrolls; `None` keeps the field on one
    /// line.
    max_rows: Option<usize>,
    /// How far the text is scrolled up, in a multi-line field.
    scroll_top: Pixels,
    /// The cursor moved: the next paint scrolls it into view.
    reveal_cursor: bool,
    /// Up, Down, Enter and Tab go to the owner's menu.
    menu_open: bool,
}

impl EventEmitter<Submitted> for TextInput {}
impl EventEmitter<PastedImage> for TextInput {}
impl EventEmitter<MenuKey> for TextInput {}

impl TextInput {
    pub fn new(placeholder: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: SharedString::default(),
            placeholder: placeholder.into(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_bounds: None,
            is_selecting: false,
            clear_on_submit: true,
            compact: false,
            submit_empty: false,
            max_rows: None,
            scroll_top: Pixels::ZERO,
            reveal_cursor: false,
            menu_open: false,
        }
    }

    pub fn submit_empty(mut self) -> Self {
        self.submit_empty = true;
        self
    }

    pub fn keep_on_submit(mut self) -> Self {
        self.clear_on_submit = false;
        self
    }

    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// Wraps long lines, takes Shift+Enter as a line break, and grows up to
    /// `max_rows` rows before it scrolls.
    pub fn multiline(mut self, max_rows: usize) -> Self {
        self.max_rows = Some(max_rows.max(1));
        self
    }

    pub fn set_placeholder(&mut self, placeholder: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.placeholder = placeholder.into();
        cx.notify();
    }

    pub fn text(&self) -> &str {
        &self.content
    }

    /// Where the cursor is, as a byte offset into [`Self::text`].
    pub fn cursor(&self) -> usize {
        self.cursor_offset()
    }

    /// Whether text is being composed by an input method; its keys are not
    /// ours to take.
    pub fn is_composing(&self) -> bool {
        self.marked_range.is_some()
    }

    /// Sends Up, Down, Enter and Tab to the owner as [`MenuKey`]s while on.
    pub fn set_menu_open(&mut self, open: bool) {
        self.menu_open = open;
    }

    /// Replaces the content and puts the cursor at the end.
    pub fn set_text(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.reset();
        self.content = text.into();
        let end = self.content.len();
        self.selected_range = end..end;
        self.reveal_cursor = true;
        cx.notify();
    }

    /// Replaces `range` of the content with `text`, leaving the cursor
    /// after it, as when an autocomplete entry is taken.
    pub fn replace(&mut self, range: Range<usize>, text: &str, cx: &mut Context<Self>) {
        let range = range.start.min(self.content.len())..range.end.min(self.content.len());
        self.content = (self.content[..range.start].to_owned() + text + &self.content[range.end..]).into();
        let cursor = range.start + text.len();
        self.selected_range = cursor..cursor;
        self.selection_reversed = false;
        self.marked_range = None;
        self.reveal_cursor = true;
        cx.notify();
    }
}

impl Render for TextInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .key_context("TextInput")
            .track_focus(&self.focus_handle(cx))
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::show_character_palette))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::submit))
            .on_action(cx.listener(Self::newline))
            .on_action(cx.listener(Self::tab))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .when(self.is_multiline(), |input| input.on_scroll_wheel(cx.listener(Self::on_scroll_wheel)))
            .w_full()
            .rounded(theme::radius::control())
            .border_1()
            .border_color(theme::border())
            .map(|input| {
                if self.compact {
                    input
                        .px(zoomed(6.))
                        .py(zoomed(4.))
                        .bg(rgb(theme::base_lower()))
                        .line_height(zoomed(18.))
                        .text_size(zoomed(13.))
                } else {
                    input
                        .px_4()
                        .py(zoomed(11.))
                        .bg(rgb(theme::surface_high()))
                        .line_height(zoomed(22.))
                        .text_size(zoomed(16.))
                }
            })
            .child(TextElement { input: cx.entity() })
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
