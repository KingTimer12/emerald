//! Message text: Discord markdown turned into styled, clickable text.
//! Custom emoji are painted over room kept for them in the text, since
//! gpui text cannot hold images.

use std::ops::Range;

use gpui::{
    AnyElement, Bounds, FontStyle, FontWeight, HighlightStyle, InteractiveText, SharedString,
    StrikethroughStyle, StyledText, TextLayout, UnderlineStyle, canvas, div, point, prelude::*, px, rgb, size,
};

use super::super::super::markdown::{self, Mention, Style};
use super::super::super::theme;
use super::super::Emerald;
use crate::discord::model::{Snowflake, custom_url};

/// Emoji-only messages, as large as Discord draws them.
const JUMBO: f32 = 48.;

impl Emerald {
    /// Message text with markdown and mentions resolved against what this
    /// client knows; links open in the browser.
    pub(super) fn rich_text(
        &self,
        id: impl Into<gpui::ElementId>,
        content: &str,
        mentions: &[(Snowflake, SharedString)],
        clickable: bool,
    ) -> AnyElement {
        self.render_rich(id, content, mentions, clickable, None, false)
    }

    /// A message's own text: [`Self::rich_text`] followed by a muted `note`,
    /// as "(edited)" trails the text it marks, and large when only emoji.
    pub(super) fn rich_text_noted(
        &self,
        id: impl Into<gpui::ElementId>,
        content: &str,
        mentions: &[(Snowflake, SharedString)],
        clickable: bool,
        note: Option<&str>,
    ) -> AnyElement {
        self.render_rich(id, content, mentions, clickable, note, true)
    }

    fn render_rich(
        &self,
        id: impl Into<gpui::ElementId>,
        content: &str,
        mentions: &[(Snowflake, SharedString)],
        clickable: bool,
        note: Option<&str>,
        may_jumbo: bool,
    ) -> AnyElement {
        let guild = self.spaces.current().info();
        let name = |mention: Mention| match mention {
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
        };
        let mut rich = markdown::parse(content, &name);
        let jumbo = may_jumbo && rich.jumbo();
        // Large emoji keep their note at text size, beside them.
        let (note, jumbo_note) = if jumbo { (None, note) } else { (note, None) };
        if let Some(note) = note {
            rich.text.push(' ');
            let start = rich.text.len();
            rich.text.push_str(note);
            rich.spans.push((start..rich.text.len(), Style::Muted));
        }
        let links = rich.links;
        let styled = StyledText::new(rich.text).with_highlights(flatten(&rich.spans));
        let layout = styled.layout().clone();
        let text = if !clickable || links.is_empty() {
            styled.into_any_element()
        } else {
            let ranges = links.iter().map(|(range, _)| range.clone()).collect();
            InteractiveText::new(id, styled)
                .on_click(ranges, move |ix, _, cx| cx.open_url(&links[ix].1))
                .into_any_element()
        };
        if rich.emojis.is_empty() && !jumbo {
            return text;
        }
        let emojis: Vec<_> = rich
            .emojis
            .into_iter()
            .filter_map(|(range, id)| Some((range, self.avatar_image(&custom_url(id))?)))
            .collect();
        let drawn = div()
            .relative()
            .when(jumbo, |text| text.text_size(px(JUMBO * 0.8)).line_height(px(JUMBO + 4.)))
            .child(text)
            .child(
                canvas(
                    move |_, window, cx| {
                        emojis
                            .into_iter()
                            .filter_map(|(range, image)| {
                                let bounds = emoji_bounds(&layout, range, jumbo)?;
                                Some((bounds, image.use_render_image(window, cx)?))
                            })
                            .collect::<Vec<_>>()
                    },
                    |_, emojis, window, _| {
                        for (bounds, image) in emojis {
                            if let Err(e) = window.paint_image(bounds, Default::default(), image, 0, false) {
                                tracing::warn!("could not paint an emoji: {e}");
                            }
                        }
                    },
                )
                .absolute()
                .size_full(),
            );
        match jumbo_note {
            Some(note) => div()
                .flex()
                .items_end()
                .gap_1()
                .child(drawn)
                .child(div().pb_1().text_color(rgb(theme::TEXT_MUTED)).child(note.to_owned()))
                .into_any_element(),
            None => drawn.into_any_element(),
        }
    }
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

/// gpui wants highlights sorted and disjoint; nested markdown overlaps, so
/// split at every boundary and merge the styles covering each piece.
fn flatten(spans: &[(Range<usize>, Style)]) -> Vec<(Range<usize>, HighlightStyle)> {
    let mut cuts: Vec<usize> = spans.iter().flat_map(|(r, _)| [r.start, r.end]).collect();
    cuts.sort_unstable();
    cuts.dedup();
    cuts.windows(2)
        .filter_map(|pair| {
            let piece = pair[0]..pair[1];
            let mut style = HighlightStyle::default();
            let mut any = false;
            for (range, kind) in spans {
                if range.start <= piece.start && piece.end <= range.end {
                    any = true;
                    style = style.highlight(highlight(*kind));
                }
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
        Style::Code | Style::Spoiler => HighlightStyle {
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
    }
}

