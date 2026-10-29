//! A message's markdown blocks laid out: paragraphs, headings, lists, quotes and code.

use super::*;
use crate::zoom::zoomed;

impl Emerald {
    #[hotpath::measure]
    pub(super) fn render_blocks(&self, id: ElementId, content: &str, options: Options) -> AnyElement {
        let blocks = markdown::blocks(content, &|mention| self.mention_name(mention, options.mentions));
        // Large emoji only when the whole text is one plain paragraph of them.
        let jumbo = options.own.is_some()
            && matches!(blocks.as_slice(), [Block::Text(TextKind::Paragraph, rich)] if rich.jumbo());
        if jumbo {
            let Some(Block::Text(_, rich)) = blocks.into_iter().next() else { unreachable!() };
            return self.render_jumbo(id, rich, &options);
        }
        let mut counter = 0;
        let mut shown = self.render_block_list(&id, blocks, &options, &mut counter);
        // The note goes on the last line of text, or under a closing block.
        if let Some(note) = options.note {
            match shown.last_mut() {
                Some(Shown::Text(rich, _, _)) => {
                    rich.text.push(' ');
                    let start = rich.text.len();
                    rich.text.push_str(note);
                    rich.spans.push((start..rich.text.len(), Style::Muted));
                }
                _ => shown.push(Shown::Element(
                    div().text_size(zoomed(12.)).text_color(theme::muted()).child(note.to_owned()).into_any_element(),
                )),
            }
        }
        if let [Shown::Text(..)] = shown.as_slice() {
            let Some(Shown::Text(rich, part, wrap)) = shown.pop() else { unreachable!() };
            return wrap(self.render_piece(id, rich, part, &options));
        }
        div()
            .flex()
            .flex_col()
            .gap(zoomed(2.))
            .min_w(zoomed(0.))
            .children(shown.into_iter().map(|shown| match shown {
                Shown::Text(rich, part, wrap) => wrap(self.render_piece(id.clone(), rich, part, &options)),
                Shown::Element(element) => element,
            }))
            .into_any_element()
    }

    /// Lays out `blocks`, leaving their text for [`Self::render_piece`] so a
    /// note can still be added to the last one. `counter` numbers the text
    /// pieces of the whole message, for selection and spoilers.
    pub(super) fn render_block_list(&self, id: &ElementId, blocks: Vec<Block>, options: &Options, counter: &mut usize) -> Vec<Shown> {
        let mut shown = Vec::new();
        for block in blocks {
            match block {
                Block::Text(kind, rich) => {
                    let part = next(counter);
                    let wrap: Wrap = match kind {
                        TextKind::Paragraph => Box::new(|text| text),
                        TextKind::Heading(level) => {
                            let size = match level {
                                1 => 24.,
                                2 => 20.,
                                _ => 16.,
                            };
                            Box::new(move |text| {
                                div()
                                    .mt(zoomed(if level == 1 { 8. } else { 4. }))
                                    .text_size(zoomed(size))
                                    .line_height(zoomed((size * 1.375).round()))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(theme::text_strong()))
                                    .child(text)
                                    .into_any_element()
                            })
                        }
                        TextKind::Subtext => Box::new(|text| {
                            div()
                                .text_size(zoomed(12.))
                                .line_height(zoomed(16.))
                                .text_color(theme::muted())
                                .child(text)
                                .into_any_element()
                        }),
                    };
                    shown.push(Shown::Text(rich, part, wrap));
                }
                Block::Item { depth, marker, rich } => {
                    let part = next(counter);
                    let indent = 4. + depth as f32 * 20.;
                    let marker: SharedString = marker.into();
                    shown.push(Shown::Text(
                        rich,
                        part,
                        Box::new(move |text| {
                            div()
                                .flex()
                                .pl(zoomed(indent))
                                .child(div().flex_none().min_w(zoomed(18.)).pr_1().child(marker))
                                .child(div().flex_1().min_w(zoomed(0.)).child(text))
                                .into_any_element()
                        }),
                    ));
                }
                Block::Quote(inner) => {
                    let inner = self.render_block_list(id, inner, options, counter);
                    let body = inner.into_iter().map(|shown| match shown {
                        Shown::Text(rich, part, wrap) => wrap(self.render_piece(id.clone(), rich, part, options)),
                        Shown::Element(element) => element,
                    });
                    shown.push(Shown::Element(
                        div()
                            .flex()
                            .child(div().flex_none().w(zoomed(4.)).rounded_sm().bg(rgb(theme::interactive_muted())))
                            .child(div().flex().flex_col().flex_1().min_w(zoomed(0.)).pl_3().children(body))
                            .into_any_element(),
                    ));
                }
                Block::Code(text) => {
                    let part = next(counter);
                    shown.push(Shown::Element(self.render_code(id, text, part, options)));
                }
            }
        }
        shown
    }

    /// A fenced code block: monospace on a bordered dark box, selectable
    /// like the rest of the message.
    pub(super) fn render_code(&self, id: &ElementId, text: String, part: usize, options: &Options) -> AnyElement {
        let text: SharedString = text.into();
        let styled = StyledText::new(text.clone());
        let layer = options.own.map(|message| {
            let selectable = Selectable {
                message,
                part,
                view: self.weak.clone(),
            };
            let selected = self.selected_range(message, part, &text);
            (select::selection_layer(selectable, text.clone(), styled.layout().clone()), selected)
        });
        let (layer, selected) = layer.map_or((None, None), |(layer, selected)| (Some(layer), selected));
        let styled = match selected {
            Some(range) => styled.with_highlights([(range, selection_highlight())]),
            None => styled,
        };
        div()
            .id(ElementId::NamedChild(Box::new(id.clone()), format!("code-{part}").into()))
            .max_w(zoomed(900.))
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(theme::border())
            .bg(theme::code_bg())
            .font_family(theme::MONO)
            .text_size(zoomed(14.))
            .line_height(zoomed(18.))
            .overflow_x_scroll()
            .child(div().relative().children(layer).child(styled))
            .into_any_element()
    }
}

/// A text block of a message, laid out but with its text not yet drawn,
/// and the wrapper that places the drawn text.
pub(super) enum Shown {
    Text(Rich, usize, Wrap),
    Element(AnyElement),
}

pub(super) type Wrap = Box<dyn FnOnce(AnyElement) -> AnyElement>;

pub(super) fn next(counter: &mut usize) -> usize {
    *counter += 1;
    *counter - 1
}
