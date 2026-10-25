//! The element that lays the text out, scrolls it and paints it with the cursor and selection.

use super::*;

pub(super) struct TextElement {
    pub(super) input: Entity<TextInput>,
}

pub(super) struct PrepaintState {
    pub(super) layout: Option<Layout>,
    /// Where the text's first row is drawn, scrolling included.
    pub(super) origin: Point<Pixels>,
    pub(super) cursor: Option<PaintQuad>,
    pub(super) selection: Vec<PaintQuad>,
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
