//! Keys and mouse: moving, selecting, typing, the clipboard and submitting.

use super::*;

impl TextInput {
    pub(super) fn is_multiline(&self) -> bool {
        self.max_rows.is_some()
    }

    pub(super) fn submit(&mut self, _: &Submit, _: &mut Window, cx: &mut Context<Self>) {
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
    pub(super) fn newline(&mut self, _: &Newline, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_multiline() {
            self.replace_text_in_range(None, "\n", window, cx);
        } else {
            self.submit(&Submit, window, cx);
        }
    }

    pub(super) fn tab(&mut self, _: &Tab, _: &mut Window, cx: &mut Context<Self>) {
        if self.menu_open {
            cx.emit(MenuKey::Accept);
        } else {
            cx.propagate();
        }
    }

    pub(super) fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx)
        }
    }

    pub(super) fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx)
        }
    }

    /// The row above. On the first row the cursor goes to the start, and
    /// once there the key is left to the window: Up edits the last message.
    pub(super) fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
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

    pub(super) fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
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

    pub(super) fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        let target = self.vertical_target(self.cursor_offset(), -1).unwrap_or(0);
        self.select_to(target, cx);
    }

    pub(super) fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        let target = self.vertical_target(self.cursor_offset(), 1).unwrap_or(self.content.len());
        self.select_to(target, cx);
    }

    /// The offset one row above (`rows` = -1) or below (1) `offset`, at the
    /// same x; `None` when there is no such row.
    pub(super) fn vertical_target(&self, offset: usize, rows: i32) -> Option<usize> {
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

    pub(super) fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    pub(super) fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    pub(super) fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx)
    }

    /// Start of the cursor's line.
    pub(super) fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        let start = line_start(&self.content, self.cursor_offset());
        self.move_to(start, cx);
    }

    /// End of the cursor's line.
    pub(super) fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        let end = line_end(&self.content, self.cursor_offset());
        self.move_to(end, cx);
    }

    pub(super) fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    pub(super) fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    pub(super) fn on_mouse_down(
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

    pub(super) fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    pub(super) fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    /// Scrolls a multi-line field whose text is taller than the box.
    pub(super) fn on_scroll_wheel(&mut self, event: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
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

    pub(super) fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    pub(super) fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
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

    pub(super) fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        } else {
            // Nothing selected here: text selected elsewhere (a message) may be.
            cx.propagate();
        }
    }
    pub(super) fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx)
        }
    }

    pub(super) fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        self.reveal_cursor = true;
        cx.notify()
    }
}
