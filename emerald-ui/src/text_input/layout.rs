//! Shaping the text for drawing: the displayed runs, wrapped lines and their geometry.

use super::*;

impl TextInput {
    /// The text drawn, the placeholder when empty, and its styled runs.
    pub(super) fn display(&self, window: &Window) -> (SharedString, Vec<TextRun>) {
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

/// Text shaped for drawing: one wrapped line per line of the text.
#[derive(Clone)]
pub(super) struct Layout {
    pub(super) lines: Vec<WrappedLine>,
    /// Byte offset of each line in the text.
    pub(super) starts: Vec<usize>,
    pub(super) line_height: Pixels,
}

impl Layout {
    pub(super) fn rows(line: &WrappedLine) -> usize {
        line.wrap_boundaries.len() + 1
    }

    pub(super) fn height(&self) -> Pixels {
        self.line_height * self.lines.iter().map(Self::rows).sum::<usize>() as f32
    }

    /// The top of each line, relative to the text's top.
    pub(super) fn tops(&self) -> impl Iterator<Item = Pixels> + '_ {
        self.lines.iter().scan(Pixels::ZERO, |top, line| {
            let this = *top;
            *top += self.line_height * Self::rows(line) as f32;
            Some(this)
        })
    }

    /// Where row `row` of `line` starts, as a byte offset in the line.
    pub(super) fn row_starts(line: &WrappedLine) -> impl Iterator<Item = usize> + '_ {
        std::iter::once(0).chain(line.wrap_boundaries.iter().map(|boundary| {
            line.unwrapped_layout.runs[boundary.run_ix].glyphs[boundary.glyph_ix].index
        }))
    }

    pub(super) fn line_of(&self, offset: usize) -> usize {
        self.starts.partition_point(|&start| start <= offset).saturating_sub(1)
    }

    /// The top-left corner of the cursor at `offset`.
    pub(super) fn position_for_index(&self, offset: usize) -> Point<Pixels> {
        let ix = self.line_of(offset);
        let Some((line, top)) = self.lines.get(ix).zip(self.tops().nth(ix)) else {
            return Point::default();
        };
        let local = (offset - self.starts[ix]).min(line.len());
        let at = line.position_for_index(local, self.line_height).unwrap_or_default();
        point(at.x, top + at.y)
    }

    pub(super) fn index_for_position(&self, position: Point<Pixels>) -> usize {
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
    pub(super) fn selection(&self, range: &Range<usize>) -> Vec<Bounds<Pixels>> {
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
