//! Message text: Discord markdown turned into styled, clickable text.

use std::ops::Range;

use gpui::{
    AnyElement, FontStyle, FontWeight, HighlightStyle, InteractiveText, SharedString, StrikethroughStyle,
    StyledText, UnderlineStyle, prelude::*, px, rgb,
};

use super::super::super::markdown::{self, Mention, Style};
use super::super::super::theme;
use super::super::Emerald;
use crate::discord::model::Snowflake;

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
        let guild = self.spaces.current().info();
        let name = |mention: Mention| match mention {
            Mention::User(id) => guild
                .and_then(|g| g.member_style(id).0.map(str::to_owned))
                .or_else(|| mentions.iter().find(|(m, _)| *m == id).map(|(_, n)| n.to_string()))
                .or_else(|| self.people.display_name(id).map(str::to_owned))
                .unwrap_or_else(|| "usuário-desconhecido".to_owned()),
            Mention::Channel(id) => self
                .spaces
                .channel(id)
                .and_then(|c| c.name.clone())
                .unwrap_or_else(|| "canal-desconhecido".to_owned()),
            Mention::Role(id) => guild
                .and_then(|g| g.role(id))
                .map_or_else(|| "cargo-desconhecido".to_owned(), |r| r.name.clone()),
        };
        let rich = markdown::parse(content, &name);
        let links = rich.links;
        let styled = StyledText::new(rich.text).with_highlights(flatten(&rich.spans));
        if !clickable || links.is_empty() {
            return styled.into_any_element();
        }
        let ranges = links.iter().map(|(range, _)| range.clone()).collect();
        InteractiveText::new(id, styled)
            .on_click(ranges, move |ix, _, cx| cx.open_url(&links[ix].1))
            .into_any_element()
    }
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
        Style::Mention => HighlightStyle {
            color: Some(rgb(theme::MENTION_TEXT).into()),
            background_color: Some(theme::mention_bg().into()),
            font_weight: Some(FontWeight::MEDIUM),
            ..Default::default()
        },
    }
}

