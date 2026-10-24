//! Text input, adapted from GPUI's `examples/input.rs` (IME, selection,
//! clipboard) with a submit action and a dark theme. Single-line by
//! default; the message box is multi-line: Shift+Enter breaks the line,
//! long lines wrap, and the box grows up to a limit, then scrolls.

use std::ops::Range;

use gpui::{
    App, AvailableSpace, Bounds, ClipboardItem, ContentMask, Context, CursorStyle, ElementId, ElementInputHandler,
    Entity, EntityInputHandler, EventEmitter, FocusHandle, Focusable, GlobalElementId, KeyBinding, LayoutId,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, ScrollWheelEvent,
    SharedString, Style, TextAlign, TextRun, UTF16Selection, UnderlineStyle, Window, WrappedLine, actions, div, fill,
    point, prelude::*, px, relative, rgb, rgba, size,
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

    fn is_multiline(&self) -> bool {
        self.max_rows.is_some()
    }

    fn submit(&mut self, _: &Submit, _: &mut Window, cx: &mut Context<Self>) {
        if self.marked_range.is_some() {
            return;
        }
        if self.menu_open {
            cx.emit(MenuKey::Accept);
            return;
        }
        let text = self.content.trim().to_owned();
        if text.is_empty() && !self.submit_empty {
            return;
        }
        if self.clear_on_submit {
            self.reset();
        }
        cx.emit(Submitted(text));
        cx.notify();
    }

    /// Shift+Enter: a line break where the field has lines, else a submit.
    fn newline(&mut self, _: &Newline, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_multiline() {
            self.replace_text_in_range(None, "\n", window, cx);
        } else {
            self.submit(&Submit, window, cx);
        }
    }

    fn tab(&mut self, _: &Tab, _: &mut Window, cx: &mut Context<Self>) {
        if self.menu_open {
            cx.emit(MenuKey::Accept);
        } else {
            cx.propagate();
        }
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx)
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx)
        }
    }

    /// The row above. On the first row the cursor goes to the start, and
    /// once there the key is left to the window: Up edits the last message.
    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        if self.menu_open {
            cx.emit(MenuKey::Up);
            return;
        }
        match self.vertical_target(self.cursor_offset(), -1) {
            Some(target) => self.move_to(target, cx),
            None if self.cursor_offset() > 0 && self.is_multiline() => self.move_to(0, cx),
            None => cx.propagate(),
        }
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        if self.menu_open {
            cx.emit(MenuKey::Down);
            return;
        }
        let end = self.content.len();
        match self.vertical_target(self.cursor_offset(), 1) {
            Some(target) => self.move_to(target, cx),
            None if self.cursor_offset() < end && self.is_multiline() => self.move_to(end, cx),
            None => cx.propagate(),
        }
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        let target = self.vertical_target(self.cursor_offset(), -1).unwrap_or(0);
        self.select_to(target, cx);
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        let target = self.vertical_target(self.cursor_offset(), 1).unwrap_or(self.content.len());
        self.select_to(target, cx);
    }

    /// The offset one row above (`rows` = -1) or below (1) `offset`, at the
    /// same x; `None` when there is no such row.
    fn vertical_target(&self, offset: usize, rows: i32) -> Option<usize> {
        if !self.is_multiline() || self.content.is_empty() {
            return None;
        }
        let layout = self.last_layout.as_ref()?;
        let at = layout.position_for_index(offset);
        let y = at.y + layout.line_height * (rows as f32 + 0.5);
        if y < Pixels::ZERO || y >= layout.height() {
            return None;
        }
        Some(layout.index_for_position(point(at.x, y)).min(self.content.len()))
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx)
    }

    /// Start of the cursor's line.
    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        let start = line_start(&self.content, self.cursor_offset());
        self.move_to(start, cx);
    }

    /// End of the cursor's line.
    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        let end = line_end(&self.content, self.cursor_offset());
        self.move_to(end, cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_selecting = true;

        if event.modifiers.shift {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        } else {
            self.move_to(self.index_for_mouse_position(event.position), cx)
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    /// Scrolls a multi-line field whose text is taller than the box.
    fn on_scroll_wheel(&mut self, event: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let (Some(layout), Some(bounds)) = (&self.last_layout, self.last_bounds) else { return };
        let overflow = layout.height() - bounds.size.height;
        if overflow <= Pixels::ZERO {
            return;
        }
        let delta = event.delta.pixel_delta(layout.line_height).y;
        self.scroll_top = (self.scroll_top - delta).clamp(Pixels::ZERO, overflow);
        cx.stop_propagation();
        cx.notify();
    }

    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = cx.read_from_clipboard() else { return };
        if let Some(text) = item.text() {
            let text = text.replace("\r\n", "\n").replace('\r', "\n");
            let text = if self.is_multiline() { text } else { text.replace('\n', " ") };
            self.replace_text_in_range(None, &text, window, cx);
        } else if let Some(gpui::ClipboardEntry::Image(image)) =
            item.entries().iter().find(|e| matches!(e, gpui::ClipboardEntry::Image(_)))
        {
            cx.emit(PastedImage(image.clone()));
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        } else {
            // Nothing selected here: text selected elsewhere (a message) may be.
            cx.propagate();
        }
    }
    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx)
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        self.reveal_cursor = true;
        cx.notify()
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }
        let (Some(bounds), Some(layout)) = (self.last_bounds.as_ref(), self.last_layout.as_ref()) else {
            return 0;
        };
        if position.y < bounds.top() && !self.is_multiline() {
            return 0;
        }
        if position.y > bounds.bottom() && !self.is_multiline() {
            return self.content.len();
        }
        let local = point(position.x - bounds.left(), position.y - bounds.top() + self.scroll_top);
        layout.index_for_position(local).min(self.content.len())
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset
        } else {
            self.selected_range.end = offset
        };
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        self.reveal_cursor = true;
        cx.notify()
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;

        for ch in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }

        utf8_offset
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;

        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }

        utf16_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }

    fn reset(&mut self) {
        self.content = "".into();
        self.selected_range = 0..0;
        self.selection_reversed = false;
        self.marked_range = None;
        self.last_layout = None;
        self.last_bounds = None;
        self.is_selecting = false;
        self.scroll_top = Pixels::ZERO;
    }

    /// The text drawn, the placeholder when empty, and its styled runs.
    fn display(&self, window: &Window) -> (SharedString, Vec<TextRun>) {
        let style = window.text_style();
        let (text, color) = if self.content.is_empty() {
            (self.placeholder.clone(), rgb(theme::PLACEHOLDER).into())
        } else {
            (self.content.clone(), style.color)
        };
        let run = TextRun {
            len: text.len(),
            font: style.font(),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = match self.marked_range.as_ref().filter(|_| !self.content.is_empty()) {
            Some(marked) => [
                TextRun {
                    len: marked.start,
                    ..run.clone()
                },
                TextRun {
                    len: marked.end - marked.start,
                    underline: Some(UnderlineStyle {
                        color: Some(run.color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: text.len() - marked.end,
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect(),
            None => vec![run],
        };
        (text, runs)
    }
}

/// Byte offset where the line holding `offset` starts.
fn line_start(text: &str, offset: usize) -> usize {
    text[..offset.min(text.len())].rfind('\n').map_or(0, |ix| ix + 1)
}

/// Byte offset where the line holding `offset` ends, before its break.
fn line_end(text: &str, offset: usize) -> usize {
    let offset = offset.min(text.len());
    text[offset..].find('\n').map_or(text.len(), |ix| offset + ix)
}

/// Where each line of `text` starts, as gpui shapes one line per `\n`.
fn line_starts(text: &str) -> Vec<usize> {
    std::iter::once(0).chain(text.match_indices('\n').map(|(ix, _)| ix + 1)).collect()
}

/// Text shaped for drawing: one wrapped line per line of the text.
#[derive(Clone)]
struct Layout {
    lines: Vec<WrappedLine>,
    /// Byte offset of each line in the text.
    starts: Vec<usize>,
    line_height: Pixels,
}

impl Layout {
    fn rows(line: &WrappedLine) -> usize {
        line.wrap_boundaries.len() + 1
    }

    fn height(&self) -> Pixels {
        self.line_height * self.lines.iter().map(Self::rows).sum::<usize>() as f32
    }

    /// The top of each line, relative to the text's top.
    fn tops(&self) -> impl Iterator<Item = Pixels> + '_ {
        self.lines.iter().scan(Pixels::ZERO, |top, line| {
            let this = *top;
            *top += self.line_height * Self::rows(line) as f32;
            Some(this)
        })
    }

    /// Where row `row` of `line` starts, as a byte offset in the line.
    fn row_starts(line: &WrappedLine) -> impl Iterator<Item = usize> + '_ {
        std::iter::once(0).chain(line.wrap_boundaries.iter().map(|boundary| {
            line.unwrapped_layout.runs[boundary.run_ix].glyphs[boundary.glyph_ix].index
        }))
    }

    fn line_of(&self, offset: usize) -> usize {
        self.starts.partition_point(|&start| start <= offset).saturating_sub(1)
    }

    /// The top-left corner of the cursor at `offset`.
    fn position_for_index(&self, offset: usize) -> Point<Pixels> {
        let ix = self.line_of(offset);
        let Some((line, top)) = self.lines.get(ix).zip(self.tops().nth(ix)) else {
            return Point::default();
        };
        let local = (offset - self.starts[ix]).min(line.len());
        let at = line.position_for_index(local, self.line_height).unwrap_or_default();
        point(at.x, top + at.y)
    }

    fn index_for_position(&self, position: Point<Pixels>) -> usize {
        if position.y < Pixels::ZERO {
            return 0;
        }
        for ((line, top), start) in self.lines.iter().zip(self.tops()).zip(&self.starts) {
            if position.y < top + self.line_height * Self::rows(line) as f32 {
                let local = point(position.x, position.y - top);
                let (Ok(ix) | Err(ix)) = line.closest_index_for_position(local, self.line_height);
                return start + ix;
            }
        }
        self.starts.last().zip(self.lines.last()).map_or(0, |(start, line)| start + line.len())
    }

    /// Highlight quads for `range`, one per row it touches, relative to the
    /// text's top-left corner. A selected line break shows as a sliver.
    fn selection(&self, range: &Range<usize>) -> Vec<Bounds<Pixels>> {
        let mut quads = Vec::new();
        for ((line, top), &start) in self.lines.iter().zip(self.tops()).zip(&self.starts) {
            let end = start + line.len();
            if range.end < start || range.start > end {
                continue;
            }
            let rows: Vec<usize> = Self::row_starts(line).collect();
            for (row, &row_start) in rows.iter().enumerate() {
                let row_end = rows.get(row + 1).copied().unwrap_or(line.len());
                let from = range.start.saturating_sub(start).clamp(row_start, row_end);
                let to = range.end.saturating_sub(start).clamp(row_start, row_end);
                let last_row = row + 1 == rows.len();
                let takes_break = last_row && range.end > end;
                if from == to && !takes_break {
                    continue;
                }
                let origin_x = line.unwrapped_layout.x_for_index(row_start);
                let left = line.unwrapped_layout.x_for_index(from) - origin_x;
                let mut right = line.unwrapped_layout.x_for_index(to) - origin_x;
                if takes_break {
                    right += px(4.);
                }
                let y = top + self.line_height * row as f32;
                quads.push(Bounds::from_corners(point(left, y), point(right, y + self.line_height)));
            }
        }
        quads
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.selection_reversed = false;
        self.marked_range.take();
        self.reveal_cursor = true;
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        if !new_text.is_empty() {
            self.marked_range = Some(range.start..range.start + new_text.len());
        } else {
            self.marked_range = None;
        }
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .map(|new_range| new_range.start + range.start..new_range.end + range.end)
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());
        self.reveal_cursor = true;

        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let layout = self.last_layout.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        let origin = point(bounds.left(), bounds.top() - self.scroll_top);
        let start = layout.position_for_index(range.start);
        let end = layout.position_for_index(range.end);
        // A range over several rows is placed by its first one.
        let right = if end.y == start.y { end.x } else { start.x };
        Some(Bounds::from_corners(
            origin + start,
            origin + point(right, start.y + layout.line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let bounds = self.last_bounds?;
        let layout = self.last_layout.as_ref()?;
        let local = gpui::point(point.x - bounds.left(), point.y - bounds.top() + self.scroll_top);
        let utf8_index = layout.index_for_position(local).min(self.content.len());
        Some(self.offset_to_utf16(utf8_index))
    }
}

struct TextElement {
    input: Entity<TextInput>,
}

struct PrepaintState {
    layout: Option<Layout>,
    /// Where the text's first row is drawn, scrolling included.
    origin: Point<Pixels>,
    cursor: Option<PaintQuad>,
    selection: Vec<PaintQuad>,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        let line_height = window.line_height();
        let input = self.input.read(cx);
        let Some(max_rows) = input.max_rows else {
            style.size.height = line_height.into();
            return (window.request_layout(style, [], cx), ());
        };
        // As tall as the wrapped text, up to `max_rows` rows.
        let (text, runs) = input.display(window);
        let font_size = window.text_style().font_size.to_pixels(window.rem_size());
        let hard_lines = text.split('\n').count();
        let layout_id = window.request_measured_layout(style, move |known, available, window, _| {
            let width = known.width.or(match available.width {
                AvailableSpace::Definite(width) => Some(width),
                _ => None,
            });
            let rows = width
                .and_then(|width| {
                    window.text_system().shape_text(text.clone(), font_size, &runs, Some(width), None).ok()
                })
                .map_or(hard_lines, |lines| lines.iter().map(Layout::rows).sum());
            size(width.unwrap_or_default(), line_height * rows.clamp(1, max_rows) as f32)
        });
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let (text, runs) = input.display(window);
        let multiline = input.is_multiline();
        let line_height = window.line_height();
        let font_size = window.text_style().font_size.to_pixels(window.rem_size());
        let wrap_width = multiline.then_some(bounds.size.width);
        let starts = line_starts(&text);
        let lines = window
            .text_system()
            .shape_text(text, font_size, &runs, wrap_width, None)
            .map(|lines| lines.into_vec())
            .unwrap_or_default();
        let layout = Layout {
            lines,
            starts,
            line_height,
        };

        // Keep the cursor in view after it moved; otherwise leave the
        // scroll where the wheel put it, within the text.
        let cursor_at = layout.position_for_index(input.cursor_offset());
        let overflow = (layout.height() - bounds.size.height).max(Pixels::ZERO);
        let mut scroll_top = input.scroll_top;
        if input.reveal_cursor {
            if cursor_at.y < scroll_top {
                scroll_top = cursor_at.y;
            } else if cursor_at.y + line_height > scroll_top + bounds.size.height {
                scroll_top = cursor_at.y + line_height - bounds.size.height;
            }
        }
        let scroll_top = scroll_top.clamp(Pixels::ZERO, overflow);
        let origin = point(bounds.left(), bounds.top() - scroll_top);

        let selected_range = input.selected_range.clone();
        let (selection, cursor) = if selected_range.is_empty() {
            let cursor = fill(Bounds::new(origin + cursor_at, size(px(2.), line_height)), theme::text());
            (Vec::new(), Some(cursor))
        } else {
            let quads = layout
                .selection(&selected_range)
                .into_iter()
                .map(|quad| fill(Bounds::new(origin + quad.origin, quad.size), rgba(0x5865f260)))
                .collect();
            (quads, None)
        };
        self.input.update(cx, |input, _| {
            input.scroll_top = scroll_top;
            input.reveal_cursor = false;
        });
        PrepaintState {
            layout: Some(layout),
            origin,
            cursor,
            selection,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        let Some(layout) = prepaint.layout.take() else { return };
        let origin = prepaint.origin;
        let cursor = prepaint.cursor.take();
        let selection = std::mem::take(&mut prepaint.selection);
        // Single-line fields keep drawing past their end, as before.
        let mask = self.input.read(cx).is_multiline().then_some(ContentMask { bounds });
        window.with_content_mask(mask, |window| {
            for quad in selection {
                window.paint_quad(quad);
            }
            for (line, top) in layout.lines.iter().zip(layout.tops()) {
                line.paint(
                    origin + point(Pixels::ZERO, top),
                    layout.line_height,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                )
                .ok();
            }
            if focus_handle.is_focused(window)
                && let Some(cursor) = cursor
            {
                window.paint_quad(cursor);
            }
        });

        self.input.update(cx, |input, _cx| {
            input.last_layout = Some(layout);
            input.last_bounds = Some(bounds);
        });
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
            .rounded_lg()
            .border_1()
            .border_color(theme::border())
            .map(|input| {
                if self.compact {
                    input
                        .px(px(6.))
                        .py(px(4.))
                        .bg(rgb(theme::BASE_LOWER))
                        .line_height(px(18.))
                        .text_size(px(13.))
                } else {
                    input
                        .px_4()
                        .py(px(11.))
                        .bg(rgb(theme::SURFACE_HIGH))
                        .line_height(px(22.))
                        .text_size(px(16.))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_found_around_the_cursor() {
        let text = "one\ntwo\n\nfour";
        assert_eq!(line_starts(text), vec![0, 4, 8, 9]);
        assert_eq!(line_start(text, 6), 4);
        assert_eq!(line_end(text, 6), 7);
        // On an empty line, both ends are the cursor.
        assert_eq!((line_start(text, 8), line_end(text, 8)), (8, 8));
        assert_eq!(line_end(text, 10), text.len());
        assert_eq!(line_start("abc", 2), 0);
        assert_eq!(line_starts("abc\n"), vec![0, 4]);
    }
}
