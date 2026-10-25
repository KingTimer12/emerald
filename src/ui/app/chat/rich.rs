//! Message text: Discord markdown turned into blocks of styled, clickable
//! text. Custom emoji are painted over room kept for them in the text,
//! since gpui text cannot hold images.

use std::ops::Range;
use std::time::Duration;

use gpui::{
    AnyElement, Bounds, ElementId, FontStyle, FontWeight, HighlightStyle, InteractiveText, SharedString,
    StrikethroughStyle, StyledText, TextLayout, UnderlineStyle, canvas, div, point, prelude::*, px, rgb, rgba, size,
};

use super::super::super::markdown::{self, Block, Mention, Rich, Style, TextKind};
use super::super::super::theme;
use super::super::Emerald;
use super::select::{self, Selectable};
use crate::discord::model::{Snowflake, animated_custom_url, custom_url};

/// Emoji-only messages, as large as Discord draws them.
const JUMBO: f32 = 48.;
/// Spoilers opened by the reader are remembered up to this many, then the
/// oldest close again.
const MAX_REVEALED: usize = 500;

/// A text spoiler the reader opened: the message, which part of it (its
/// content or one of its embeds), the block and the spoiler's place in it.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct SpoilerKey {
    message: Snowflake,
    part: u32,
    block: u32,
    spoiler: u32,
}

/// How a text is drawn and what it lets the reader do.
#[derive(Default)]
struct Options<'a> {
    mentions: &'a [(Snowflake, SharedString)],
    /// Links, mentions and spoilers respond to clicks.
    clickable: bool,
    /// Where opened spoilers are remembered; without it they stay closed.
    scope: Option<(Snowflake, u32)>,
    /// A message's own text: selectable, and large when only emoji.
    own: Option<Snowflake>,
    /// Muted words after the text, like "(edited)".
    note: Option<&'a str>,
}

/// What a click on a range of text does.
#[derive(Clone)]
enum Click {
    Reveal(SpoilerKey),
    Open(String),
    /// A `discord.com/channels/…` link, opened here.
    Link(Option<Snowflake>, Snowflake, Option<Snowflake>),
    Channel(Snowflake),
    User(Snowflake),
}

impl Emerald {
    /// One line of inline markdown, for a channel topic or a reply preview.
    /// Nothing in it responds to clicks.
    pub(super) fn rich_line(
        &self,
        id: impl Into<ElementId>,
        content: &str,
        mentions: &[(Snowflake, SharedString)],
    ) -> AnyElement {
        let options = Options {
            mentions,
            ..Options::default()
        };
        let rich = markdown::parse(content, &|mention| self.mention_name(mention, mentions));
        self.render_piece(id.into(), rich, 0, &options)
    }

    /// Message text in full, for previews: blocks drawn, nothing clickable.
    pub(super) fn rich_text(
        &self,
        id: impl Into<ElementId>,
        content: &str,
        mentions: &[(Snowflake, SharedString)],
    ) -> AnyElement {
        self.render_blocks(
            id.into(),
            content,
            Options {
                mentions,
                ..Options::default()
            },
        )
    }

    /// A text inside an embed or a forward of `message`; `part` tells
    /// them apart.
    pub(super) fn rich_embed(
        &self,
        id: impl Into<ElementId>,
        content: &str,
        mentions: &[(Snowflake, SharedString)],
        message: Snowflake,
        part: u32,
    ) -> AnyElement {
        self.render_blocks(
            id.into(),
            content,
            Options {
                mentions,
                clickable: true,
                scope: Some((message, part)),
                ..Options::default()
            },
        )
    }

    /// A message's own text: clickable, selectable by dragging over it,
    /// large when only emoji, and followed by a muted `note`, as "(edited)"
    /// trails the text it marks.
    pub(super) fn rich_text_noted(
        &self,
        id: impl Into<ElementId>,
        content: &str,
        mentions: &[(Snowflake, SharedString)],
        note: Option<&str>,
        message: Snowflake,
    ) -> AnyElement {
        self.render_blocks(
            id.into(),
            content,
            Options {
                mentions,
                clickable: true,
                scope: Some((message, 0)),
                own: Some(message),
                note,
            },
        )
    }

    /// The name a mention shows: the member's nickname in this guild, the
    /// name the message carried, or the channel's or role's name.
    fn mention_name(&self, mention: Mention, mentions: &[(Snowflake, SharedString)]) -> String {
        let guild = self.spaces.current().info();
        match mention {
            Mention::User(id) => guild
                .and_then(|g| g.member_style(id).0.map(str::to_owned))
                .or_else(|| mentions.iter().find(|(m, _)| *m == id).map(|(_, n)| n.to_string()))
                .or_else(|| self.people.display_name(id).map(str::to_owned))
                .unwrap_or_else(|| t!("chat.unknown_user").to_owned()),
            Mention::Channel(id) => self
                .spaces
                .channel(id)
                .and_then(|c| c.name.clone())
                .unwrap_or_else(|| t!("chat.unknown_channel").to_owned()),
            Mention::Role(id) => guild
                .and_then(|g| g.role(id))
                .map_or_else(|| t!("chat.unknown_role").to_owned(), |r| r.name.clone()),
        }
    }

    fn render_blocks(&self, id: ElementId, content: &str, options: Options) -> AnyElement {
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
                    div().text_size(px(12.)).text_color(theme::muted()).child(note.to_owned()).into_any_element(),
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
            .gap(px(2.))
            .min_w(px(0.))
            .children(shown.into_iter().map(|shown| match shown {
                Shown::Text(rich, part, wrap) => wrap(self.render_piece(id.clone(), rich, part, &options)),
                Shown::Element(element) => element,
            }))
            .into_any_element()
    }

    /// Lays out `blocks`, leaving their text for [`Self::render_piece`] so a
    /// note can still be added to the last one. `counter` numbers the text
    /// pieces of the whole message, for selection and spoilers.
    fn render_block_list(&self, id: &ElementId, blocks: Vec<Block>, options: &Options, counter: &mut usize) -> Vec<Shown> {
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
                                    .mt(px(if level == 1 { 8. } else { 4. }))
                                    .text_size(px(size))
                                    .line_height(px((size * 1.375).round()))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(theme::TEXT_STRONG))
                                    .child(text)
                                    .into_any_element()
                            })
                        }
                        TextKind::Subtext => Box::new(|text| {
                            div()
                                .text_size(px(12.))
                                .line_height(px(16.))
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
                                .pl(px(indent))
                                .child(div().flex_none().min_w(px(18.)).pr_1().child(marker))
                                .child(div().flex_1().min_w(px(0.)).child(text))
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
                            .child(div().flex_none().w(px(4.)).rounded_sm().bg(rgb(theme::INTERACTIVE_MUTED)))
                            .child(div().flex().flex_col().flex_1().min_w(px(0.)).pl_3().children(body))
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
    fn render_code(&self, id: &ElementId, text: String, part: usize, options: &Options) -> AnyElement {
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
            .max_w(px(900.))
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(theme::border())
            .bg(theme::code_bg())
            .font_family(theme::MONO)
            .text_size(px(14.))
            .line_height(px(18.))
            .overflow_x_scroll()
            .child(div().relative().children(layer).child(styled))
            .into_any_element()
    }

    /// The selection over piece `part` of `message`, if it holds one.
    fn selected_range(&self, message: Snowflake, part: usize, text: &str) -> Option<Range<usize>> {
        self.text_selection.as_ref()?.range_in(message, part, text)
    }

    /// One run of styled text with its clicks, painted emoji and selection.
    fn render_piece(&self, id: ElementId, rich: Rich, part: usize, options: &Options) -> AnyElement {
        self.render_text(id, rich, part, options, false)
    }

    /// An emoji-only message, drawn large with its note at text size beside.
    fn render_jumbo(&self, id: ElementId, rich: Rich, options: &Options) -> AnyElement {
        let text = self.render_text(id, rich, 0, options, true);
        match options.note {
            Some(note) => div()
                .flex()
                .items_end()
                .gap_1()
                .child(text)
                .child(div().pb_1().text_color(rgb(theme::TEXT_MUTED)).child(note.to_owned()))
                .into_any_element(),
            None => text,
        }
    }

    fn render_text(&self, id: ElementId, mut rich: Rich, part: usize, options: &Options, jumbo: bool) -> AnyElement {
        let id = ElementId::NamedChild(Box::new(id), format!("text-{part}").into());
        // Role mentions in the role's color, as the web client tints them.
        let guild = self.spaces.current().info();
        for (range, mention) in &rich.mentions {
            if let Mention::Role(role) = mention
                && let Some(color) = guild.and_then(|g| g.role(*role)).map(|r| r.color).filter(|c| *c != 0)
            {
                rich.spans.push((range.clone(), Style::Tinted(color)));
            }
        }
        let keys: Vec<Option<SpoilerKey>> = (0..rich.spoilers.len())
            .map(|n| {
                options.scope.map(|(message, scope_part)| SpoilerKey {
                    message,
                    part: scope_part,
                    block: part as u32,
                    spoiler: n as u32,
                })
            })
            .collect();
        let hidden: Vec<Range<usize>> = rich
            .spoilers
            .iter()
            .zip(&keys)
            .filter(|(_, key)| key.is_none_or(|key| !self.revealed_spoilers.contains(&key)))
            .map(|(range, _)| range.clone())
            .collect();
        let covered = |range: &Range<usize>| hidden.iter().any(|h| h.start < range.end && range.start < h.end);

        let mut clicks: Vec<(Range<usize>, Click)> = Vec::new();
        if options.clickable {
            for (range, key) in rich.spoilers.iter().zip(&keys) {
                if let Some(key) = key.filter(|key| !self.revealed_spoilers.contains(key)) {
                    clicks.push((range.clone(), Click::Reveal(key)));
                }
            }
            for (range, url) in &rich.links {
                if !covered(range) {
                    let click = match markdown::channel_link(url) {
                        Some((guild, channel, message)) => Click::Link(guild, channel, message),
                        None => Click::Open(url.clone()),
                    };
                    clicks.push((range.clone(), click));
                }
            }
            for (range, mention) in &rich.mentions {
                let click = match mention {
                    Mention::Channel(channel) => Click::Channel(*channel),
                    Mention::User(user) => Click::User(*user),
                    Mention::Role(_) => continue,
                };
                if !covered(range) {
                    clicks.push((range.clone(), click));
                }
            }
        }

        let text: SharedString = std::mem::take(&mut rich.text).into();
        let selectable = options.own.map(|message| Selectable {
            message,
            part,
            view: self.weak.clone(),
        });
        let selected = options.own.and_then(|message| self.selected_range(message, part, &text));
        let styled = StyledText::new(text.clone()).with_highlights(flatten(&rich.spans, &hidden, selected));
        let layout = styled.layout().clone();
        let shown = if clicks.is_empty() {
            styled.into_any_element()
        } else {
            let ranges = clicks.iter().map(|(range, _)| range.clone()).collect();
            let view = self.weak.clone();
            InteractiveText::new(id, styled)
                .on_click(ranges, move |ix, window, cx| match clicks[ix].1.clone() {
                    Click::Open(url) => cx.open_url(&url),
                    click => {
                        view.update(cx, |this, cx| this.on_text_click(click, window, cx)).ok();
                    }
                })
                .into_any_element()
        };
        let layer = selectable.map(|s| select::selection_layer(s, text, layout.clone()));
        let animate = self.window_active;
        let emojis: Vec<_> = rich
            .emojis
            .into_iter()
            .filter(|emoji| !covered(&emoji.range))
            .filter_map(|emoji| {
                let animated = emoji.animated && animate;
                let url = if animated { animated_custom_url(emoji.id) } else { custom_url(emoji.id) };
                let image = self.avatar_image(&url)?;
                Some((emoji.range, image, animated))
            })
            .collect();
        if emojis.is_empty() && !jumbo && layer.is_none() {
            return shown;
        }
        if emojis.iter().any(|(_, _, animated)| *animated) {
            self.animating.set(true);
        }
        let elapsed = self.animation_clock.elapsed();
        div()
            .relative()
            .when(jumbo, |text| text.text_size(px(JUMBO * 0.8)).line_height(px(JUMBO + 4.)))
            .children(layer)
            .child(shown)
            .child(
                canvas(
                    move |_, window, cx| {
                        emojis
                            .into_iter()
                            .filter_map(|(range, image, animated)| {
                                let bounds = emoji_bounds(&layout, range, jumbo)?;
                                let image = image.use_render_image(window, cx)?;
                                let frame = if animated { frame_at(&image, elapsed) } else { 0 };
                                Some((bounds, image, frame))
                            })
                            .collect::<Vec<_>>()
                    },
                    |_, emojis, window, _| {
                        for (bounds, image, frame) in emojis {
                            if let Err(e) = window.paint_image(bounds, Default::default(), image, frame, false) {
                                tracing::warn!("could not paint an emoji: {e}");
                            }
                        }
                    },
                )
                .absolute()
                .size_full(),
            )
            .into_any_element()
    }

    /// A click on a spoiler, a channel or a user in some text.
    fn on_text_click(&mut self, click: Click, window: &mut gpui::Window, cx: &mut gpui::Context<Self>) {
        match click {
            Click::Reveal(key) => {
                if self.revealed_spoilers.len() >= MAX_REVEALED {
                    self.revealed_spoilers.clear();
                }
                self.revealed_spoilers.insert(key);
                if let Some(channel) = self.selected_channel {
                    self.history.redraw(channel, key.message, super::super::shown(&self.messages, Some(channel), channel));
                }
                cx.notify();
            }
            Click::Channel(channel) => self.open_channel_link(None, channel, None, window, cx),
            Click::Link(guild, channel, message) => self.open_channel_link(guild, channel, message, window, cx),
            Click::User(user) => {
                let guild = self.spaces.current().info().map(|_| self.spaces.current().id);
                self.open_profile(user, guild, window, cx);
            }
            Click::Open(url) => cx.open_url(&url),
        }
    }
}

/// A text block of a message, laid out but with its text not yet drawn,
/// and the wrapper that places the drawn text.
enum Shown {
    Text(Rich, usize, Wrap),
    Element(AnyElement),
}

type Wrap = Box<dyn FnOnce(AnyElement) -> AnyElement>;

fn next(counter: &mut usize) -> usize {
    *counter += 1;
    *counter - 1
}

/// The frame of an animated image due `elapsed` into the animation clock,
/// following each frame's own delay.
fn frame_at(image: &gpui::RenderImage, elapsed: Duration) -> usize {
    let count = image.frame_count();
    if count <= 1 {
        return 0;
    }
    // GIFs that ask for no delay play at the browsers' 100ms.
    let delay = |ix: usize| {
        let delay = Duration::from(image.delay(ix));
        if delay < Duration::from_millis(20) { Duration::from_millis(100) } else { delay }
    };
    let total: Duration = (0..count).map(delay).sum();
    let mut at = Duration::from_nanos((elapsed.as_nanos() % total.as_nanos().max(1)) as u64);
    for ix in 0..count {
        let this = delay(ix);
        if at < this {
            return ix;
        }
        at -= this;
    }
    count - 1
}

/// Where the emoji kept at `range` goes: centred on its room, as tall as
/// Discord draws it beside text of this line height.
fn emoji_bounds(layout: &TextLayout, range: Range<usize>, jumbo: bool) -> Option<Bounds<gpui::Pixels>> {
    let start = layout.position_for_index(range.start)?;
    let end = layout.position_for_index(range.end)?;
    let line = layout.line_height();
    let side = if jumbo { px(JUMBO) } else { line * 1.1 };
    // A room split by wrapping shows the emoji where it starts.
    let room = if end.y == start.y { end.x - start.x } else { side };
    let origin = point(start.x + (room - side) / 2., start.y + (line - side) / 2.);
    Some(Bounds::new(origin, size(side, side)))
}

fn selection_highlight() -> HighlightStyle {
    HighlightStyle {
        background_color: Some(theme::text_selection().into()),
        ..Default::default()
    }
}

/// gpui wants highlights sorted and disjoint; nested markdown overlaps, so
/// split at every boundary and merge the styles covering each piece. Closed
/// spoilers cover what is under them; the selection, if any, goes on top.
fn flatten(
    spans: &[(Range<usize>, Style)],
    hidden: &[Range<usize>],
    selected: Option<Range<usize>>,
) -> Vec<(Range<usize>, HighlightStyle)> {
    let mut cuts: Vec<usize> = spans
        .iter()
        .map(|(r, _)| r)
        .chain(hidden)
        .chain(&selected)
        .flat_map(|r| [r.start, r.end])
        .collect();
    cuts.sort_unstable();
    cuts.dedup();
    let within = |outer: &Range<usize>, piece: &Range<usize>| outer.start <= piece.start && piece.end <= outer.end;
    cuts.windows(2)
        .filter_map(|pair| {
            let piece = pair[0]..pair[1];
            let mut style = HighlightStyle::default();
            let mut any = false;
            for (range, kind) in spans {
                if within(range, &piece) {
                    any = true;
                    style = style.highlight(highlight(*kind));
                }
            }
            if hidden.iter().any(|h| within(h, &piece)) {
                any = true;
                // The text takes the cover's color, so nothing shows through.
                style = style.highlight(HighlightStyle {
                    color: Some(rgb(theme::SPOILER_HIDDEN).into()),
                    background_color: Some(rgb(theme::SPOILER_HIDDEN).into()),
                    ..Default::default()
                });
            }
            if selected.as_ref().is_some_and(|s| within(s, &piece)) {
                any = true;
                style = style.highlight(selection_highlight());
            }
            any.then_some((piece, style))
        })
        .collect()
}

fn highlight(style: Style) -> HighlightStyle {
    let line = UnderlineStyle {
        thickness: px(1.),
        color: None,
        wavy: false,
    };
    match style {
        Style::Bold => HighlightStyle {
            font_weight: Some(FontWeight::BOLD),
            ..Default::default()
        },
        Style::Italic => HighlightStyle {
            font_style: Some(FontStyle::Italic),
            ..Default::default()
        },
        Style::Underline => HighlightStyle {
            underline: Some(line),
            ..Default::default()
        },
        Style::Strike => HighlightStyle {
            strikethrough: Some(StrikethroughStyle {
                thickness: px(1.),
                color: None,
            }),
            ..Default::default()
        },
        Style::Code | Style::Spoiler | Style::Timestamp => HighlightStyle {
            background_color: Some(theme::code_bg().into()),
            ..Default::default()
        },
        Style::Link => HighlightStyle {
            color: Some(rgb(theme::LINK).into()),
            ..Default::default()
        },
        // Colours blend, so only fading takes the text away.
        Style::Hidden => HighlightStyle {
            fade_out: Some(1.),
            ..Default::default()
        },
        Style::Muted => HighlightStyle {
            color: Some(rgb(theme::TEXT_MUTED).into()),
            ..Default::default()
        },
        Style::Mention => HighlightStyle {
            color: Some(rgb(theme::MENTION_TEXT).into()),
            background_color: Some(theme::mention_bg().into()),
            font_weight: Some(FontWeight::MEDIUM),
            ..Default::default()
        },
        // The role's color, on a tenth of it.
        Style::Tinted(color) => HighlightStyle {
            color: Some(rgb(color).into()),
            background_color: Some(rgba(color << 8 | 0x1a).into()),
            ..Default::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_spoilers_cover_their_text() {
        let spans = [(0..5, Style::Bold)];
        let pieces = flatten(&spans, &[2..4], None);
        let ranges: Vec<_> = pieces.iter().map(|(r, _)| r.clone()).collect();
        assert_eq!(ranges, [0..2, 2..4, 4..5]);
        let covered = &pieces[1].1;
        assert_eq!(covered.color, Some(rgb(theme::SPOILER_HIDDEN).into()));
        assert_eq!(covered.font_weight, Some(FontWeight::BOLD));
    }
}
