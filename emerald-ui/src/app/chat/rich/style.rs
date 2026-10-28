//! Markdown styles as gpui highlights, flattened into disjoint pieces.

use super::*;
use crate::zoom::zoomed;

/// gpui wants highlights sorted and disjoint; nested markdown overlaps, so
/// split at every boundary and merge the styles covering each piece. Closed
/// spoilers cover what is under them; the selection, if any, goes on top.
pub(super) fn flatten(
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
                    color: Some(rgb(theme::spoiler_hidden()).into()),
                    background_color: Some(rgb(theme::spoiler_hidden()).into()),
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

pub(super) fn highlight(style: Style) -> HighlightStyle {
    let line = UnderlineStyle {
        thickness: zoomed(1.),
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
                thickness: zoomed(1.),
                color: None,
            }),
            ..Default::default()
        },
        Style::Code | Style::Spoiler | Style::Timestamp => HighlightStyle {
            background_color: Some(theme::code_bg().into()),
            ..Default::default()
        },
        Style::Link => HighlightStyle {
            color: Some(rgb(theme::link()).into()),
            ..Default::default()
        },
        // Colours blend, so only fading takes the text away.
        Style::Hidden => HighlightStyle {
            fade_out: Some(1.),
            ..Default::default()
        },
        Style::Muted => HighlightStyle {
            color: Some(rgb(theme::text_muted()).into()),
            ..Default::default()
        },
        Style::Mention => HighlightStyle {
            color: Some(rgb(theme::mention_text()).into()),
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
        let spoiler = 2..4;
        let pieces = flatten(&spans, std::slice::from_ref(&spoiler), None);
        let ranges: Vec<_> = pieces.iter().map(|(r, _)| r.clone()).collect();
        assert_eq!(ranges, [0..2, 2..4, 4..5]);
        let covered = &pieces[1].1;
        assert_eq!(covered.color, Some(rgb(theme::spoiler_hidden()).into()));
        assert_eq!(covered.font_weight, Some(FontWeight::BOLD));
    }
}
