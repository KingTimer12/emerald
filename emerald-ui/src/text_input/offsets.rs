//! Byte, UTF-16 and grapheme offsets in the text, and lines.

use super::*;

impl TextInput {
    pub(super) fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    pub(super) fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
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

    pub(super) fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
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

    pub(super) fn offset_from_utf16(&self, offset: usize) -> usize {
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

    pub(super) fn offset_to_utf16(&self, offset: usize) -> usize {
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

    pub(super) fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    pub(super) fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    pub(super) fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    pub(super) fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }

    pub(super) fn reset(&mut self) {
        self.content = "".into();
        self.selected_range = 0..0;
        self.selection_reversed = false;
        self.marked_range = None;
        self.last_layout = None;
        self.last_bounds = None;
        self.is_selecting = false;
        self.scroll_top = Pixels::ZERO;
    }
}

/// Byte offset where the line holding `offset` starts.
pub(super) fn line_start(text: &str, offset: usize) -> usize {
    text[..offset.min(text.len())].rfind('\n').map_or(0, |ix| ix + 1)
}

/// Byte offset where the line holding `offset` ends, before its break.
pub(super) fn line_end(text: &str, offset: usize) -> usize {
    let offset = offset.min(text.len());
    text[offset..].find('\n').map_or(text.len(), |ix| offset + ix)
}

/// Where each line of `text` starts, as gpui shapes one line per `\n`.
pub(super) fn line_starts(text: &str) -> Vec<usize> {
    std::iter::once(0).chain(text.match_indices('\n').map(|(ix, _)| ix + 1)).collect()
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
