//! Discord markdown. A message splits into blocks (paragraphs, headings,
//! subtext, list items, block quotes and fenced code), and each block's
//! text into styled spans: bold, italic, underline, strikethrough, inline
//! code, spoilers, links, mentions resolved to names, timestamps and custom
//! emoji. Everything else stays literal, as Discord leaves unmatched markers.

mod blocks;
mod inline;
mod links;

use blocks::*;
pub use links::*;

use std::ops::Range;

use super::time;
use emerald_discord::model::Snowflake;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Bold,
    Italic,
    Underline,
    Strike,
    Code,
    Spoiler,
    Link,
    Mention,
    /// A `<t:…>` timestamp, shown in the reader's timezone.
    Timestamp,
    /// Never parsed: a role mention in the role's color, set by the view.
    Tinted(u32),
    /// Never parsed: for notes the view appends, like "(edited)".
    Muted,
    /// Invisible text holding room for a custom emoji drawn over it.
    Hidden,
}

/// Stands in for a custom emoji in the text so the line keeps room for its
/// image. Letters, so a line never wraps inside it.
pub const EMOJI_ROOM: &str = "Mn";

/// Messages of only emoji show them large, as in Discord, up to this many.
const JUMBO_MAX: usize = 30;
/// Custom emoji drawn as images per message; later ones show as `:name:`.
/// Each distinct one is an image to fetch and keep, and a message can name
/// hundreds, enough for a few on screen to churn the whole image cache.
const MAX_EMOJI_IMAGES: usize = 30;
/// List items deeper than this are drawn at this depth, as Discord caps them.
const MAX_LIST_DEPTH: usize = 2;

/// What a `<…>` token refers to, for the caller to name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mention {
    User(Snowflake),
    Channel(Snowflake),
    Role(Snowflake),
}

/// A custom emoji kept room for in a text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomEmoji {
    pub range: Range<usize>,
    pub id: Snowflake,
    /// `<a:…>`: drawn from its GIF, frame by frame.
    pub animated: bool,
}

#[derive(Default)]
pub struct Rich {
    pub text: String,
    pub spans: Vec<(Range<usize>, Style)>,
    /// Clickable ranges of `text` with the URL each opens.
    pub links: Vec<(Range<usize>, String)>,
    pub emojis: Vec<CustomEmoji>,
    /// Each `<@…>`, `<#…>` and `<@&…>`, for the view to color and open.
    pub mentions: Vec<(Range<usize>, Mention)>,
    /// `||text||` ranges, in order, hidden until clicked.
    pub spoilers: Vec<Range<usize>>,
}

/// How a run of text lines is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextKind {
    Paragraph,
    /// `#`, `##` or `###`: 1 to 3.
    Heading(u8),
    /// `-# `: small and muted.
    Subtext,
}

pub enum Block {
    Text(TextKind, Rich),
    /// `- item`, `* item` or `1. item`; `marker` is "•" or the number.
    Item { depth: usize, marker: String, rich: Rich },
    /// `> line`, or `>>> ` for the rest of the message.
    Quote(Vec<Block>),
    /// A fenced code block, its text kept as written. The language named
    /// on the fence is dropped: nothing is highlighted yet.
    Code(String),
}

impl Rich {
    fn link(&mut self, label: &str, url: &str) {
        let start = self.text.len();
        self.text.push_str(label);
        self.spans.push((start..self.text.len(), Style::Link));
        self.links.push((start..self.text.len(), url.to_owned()));
    }

    /// Appends `text` in `style`; the range it took.
    fn push_styled(&mut self, text: &str, style: Style) -> Range<usize> {
        let start = self.text.len();
        self.text.push_str(text);
        self.spans.push((start..self.text.len(), style));
        start..self.text.len()
    }

    fn emoji(&mut self, id: Snowflake, animated: bool) {
        let start = self.text.len();
        self.text.push_str(EMOJI_ROOM);
        self.spans.push((start..self.text.len(), Style::Hidden));
        self.emojis.push(CustomEmoji {
            range: start..self.text.len(),
            id,
            animated,
        });
    }

    /// Whether the text is nothing but emoji, few enough to show large.
    pub fn jumbo(&self) -> bool {
        let mut count = self.emojis.len();
        let mut rest = self.text.as_str();
        let mut last = 0;
        for emoji in &self.emojis {
            let range = &emoji.range;
            if !rest.is_char_boundary(range.start - last) {
                return false;
            }
            let (before, _) = rest.split_at(range.start - last);
            match unicode_emoji(before) {
                Some(n) => count += n,
                None => return false,
            }
            rest = &rest[range.end - last..];
            last = range.end;
        }
        match unicode_emoji(rest) {
            Some(n) => count += n,
            None => return false,
        }
        (1..=JUMBO_MAX).contains(&count)
    }
}

/// How many Unicode emoji `text` holds when it is only emoji and spaces.
/// Pictographs start at U+1F000 or are older symbols turned emoji; joiners,
/// variation selectors and skin tones only extend the one before.
fn unicode_emoji(text: &str) -> Option<usize> {
    let mut count = 0;
    let mut joined = false;
    for c in text.chars() {
        match c {
            c if c.is_whitespace() => joined = false,
            '\u{200d}' => joined = true,
            '\u{fe0f}' | '\u{20e3}' | '\u{1f3fb}'..='\u{1f3ff}' | '\u{e0020}'..='\u{e007f}' => {}
            c if !c.is_alphanumeric() && (c as u32 >= 0x1f000 || ('\u{2190}'..='\u{2bff}').contains(&c)) => {
                if !std::mem::take(&mut joined) {
                    count += 1;
                }
            }
            _ => return None,
        }
    }
    Some(count)
}

/// Turns text into spans, sharing the emoji image cap across the blocks of
/// one message.
struct Parser<'a> {
    name: &'a dyn Fn(Mention) -> String,
    emojis_left: usize,
}

/// `source` as inline text only, every line in one [`Rich`]: for one-line
/// places like a channel topic or a reply preview. Headings lose their
/// hashes and turn bold.
pub fn parse(source: &str, name: &dyn Fn(Mention) -> String) -> Rich {
    let mut parser = Parser {
        name,
        emojis_left: MAX_EMOJI_IMAGES,
    };
    let mut rich = Rich {
        text: String::with_capacity(source.len()),
        ..Rich::default()
    };
    for (ix, line) in source.split('\n').enumerate() {
        if ix > 0 {
            rich.text.push('\n');
        }
        let heading = ["### ", "## ", "# "].iter().find_map(|h| line.strip_prefix(h));
        let start = rich.text.len();
        parser.inline(heading.unwrap_or(line), &mut rich);
        if heading.is_some() {
            rich.spans.push((start..rich.text.len(), Style::Bold));
        }
    }
    rich
}

/// `source` split into the blocks a message draws.
pub fn blocks(source: &str, name: &dyn Fn(Mention) -> String) -> Vec<Block> {
    let mut parser = Parser {
        name,
        emojis_left: MAX_EMOJI_IMAGES,
    };
    let mut out = Vec::new();
    let mut rest = source;
    while let Some(fence) = code_fence(rest) {
        parser.lines(rest[..fence.start].strip_suffix('\n').unwrap_or(&rest[..fence.start]), false, &mut out);
        out.push(Block::Code(fence.code.to_owned()));
        rest = &rest[fence.end..];
        rest = rest.strip_prefix('\n').unwrap_or(rest);
    }
    parser.lines(rest, false, &mut out);
    out
}

#[cfg(test)]
mod tests;
