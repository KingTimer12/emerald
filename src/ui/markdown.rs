//! Discord markdown. A message splits into blocks (paragraphs, headings,
//! subtext, list items, block quotes and fenced code), and each block's
//! text into styled spans: bold, italic, underline, strikethrough, inline
//! code, spoilers, links, mentions resolved to names, timestamps and custom
//! emoji. Everything else stays literal, as Discord leaves unmatched markers.

use std::ops::Range;

use super::time;
use crate::discord::model::Snowflake;

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

/// `https://…` or `http://…` up to whitespace.
fn bare_url(source: &str) -> Option<&str> {
    if !(source.starts_with("https://") || source.starts_with("http://")) {
        return None;
    }
    Some(&source[..source.find(char::is_whitespace).unwrap_or(source.len())])
}

/// `url` when it is a whole http(s) URL, the only links a message may open.
pub fn web_url(url: &str) -> Option<&str> {
    (bare_url(url) == Some(url)).then_some(url)
}

/// What a `discord.com/channels/…` link opens: the guild (`None` for
/// `@me`, the DMs), the channel and maybe a message. The PTB and Canary
/// hosts and the old discordapp.com count too.
pub fn channel_link(url: &str) -> Option<(Option<Snowflake>, Snowflake, Option<Snowflake>)> {
    let rest = url.strip_prefix("https://")?;
    let (host, path) = rest.split_once('/')?;
    let known = ["discord.com", "discordapp.com"]
        .iter()
        .any(|base| host == *base || ["ptb.", "canary.", "www."].iter().any(|sub| host == format!("{sub}{base}")));
    if !known {
        return None;
    }
    let path = path.split(['?', '#']).next()?.trim_end_matches('/');
    let mut parts = path.strip_prefix("channels/")?.split('/');
    let id = |part: &str| part.parse().ok().map(Snowflake);
    let guild = match parts.next()? {
        "@me" => None,
        guild => Some(id(guild)?),
    };
    let channel = id(parts.next()?)?;
    let message = match parts.next() {
        Some(message) => Some(id(message)?),
        None => None,
    };
    parts.next().is_none().then_some((guild, channel, message))
}

/// The host a URL opens, past any `user@` prefix, as the browser reads it.
fn host(url: &str) -> &str {
    let authority = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = &authority[..authority.find(['/', '?', '#']).unwrap_or(authority.len())];
    authority.rsplit('@').next().unwrap_or(authority)
}

/// Paired markers, longest first so `**` wins over `*`.
const MARKERS: [(&str, Style); 7] = [
    ("```", Style::Code),
    ("||", Style::Spoiler),
    ("**", Style::Bold),
    ("__", Style::Underline),
    ("~~", Style::Strike),
    ("*", Style::Italic),
    ("_", Style::Italic),
];

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

/// A fenced code block found in some text.
struct Fence<'a> {
    /// Where the opening fence starts and past where the closing one ends.
    start: usize,
    end: usize,
    code: &'a str,
}

/// The first ```` ``` ```` pair with something between. A first line of one
/// word names the language, when more lines follow it.
fn code_fence(source: &str) -> Option<Fence<'_>> {
    let mut from = 0;
    loop {
        let start = from + source[from..].find("```")?;
        let inner_start = start + 3;
        let close = inner_start + source[inner_start..].find("```")?;
        let inner = &source[inner_start..close];
        if inner.trim().is_empty() {
            from = close;
            continue;
        }
        let code = match inner.split_once('\n') {
            Some((first, body)) if first.chars().all(|c| c.is_ascii_alphanumeric() || "+-_#.".contains(c)) => body,
            _ => inner,
        };
        return Some(Fence {
            start,
            end: close + 3,
            code: code.strip_suffix('\n').unwrap_or(code),
        });
    }
}

/// `- `, `* ` or `12. ` after any indent: the indent in spaces, the number
/// if ordered, and the item's text.
fn list_item(line: &str) -> Option<(usize, Option<u32>, &str)> {
    let text = line.trim_start_matches(' ');
    let indent = line.len() - text.len();
    if let Some(item) = text.strip_prefix("- ").or_else(|| text.strip_prefix("* ")) {
        return Some((indent, None, item));
    }
    let digits = text.find(|c: char| !c.is_ascii_digit())?;
    let item = text[digits..].strip_prefix(". ")?;
    // Long enough numbers are not lists, as in Discord.
    let number = text[..digits].parse().ok().filter(|_| (1..=9).contains(&digits))?;
    Some((indent, Some(number), item))
}

impl Parser<'_> {
    /// Text with no fences in it, line by line. `quoted` text is already
    /// inside a quote, where quote markers stay literal.
    fn lines(&mut self, text: &str, quoted: bool, out: &mut Vec<Block>) {
        if text.is_empty() {
            return;
        }
        let lines: Vec<&str> = text.split('\n').collect();
        let mut paragraph: Vec<&str> = Vec::new();
        let mut ix = 0;
        while ix < lines.len() {
            let line = lines[ix];
            if !quoted && let Some(first) = line.strip_prefix(">>> ") {
                self.flush(&mut paragraph, out);
                let mut inner = vec![first];
                inner.extend(&lines[ix + 1..]);
                let mut quote = Vec::new();
                self.lines(&inner.join("\n"), true, &mut quote);
                out.push(Block::Quote(quote));
                return;
            }
            if !quoted && (line.starts_with("> ") || line == ">") {
                self.flush(&mut paragraph, out);
                let mut inner = Vec::new();
                while let Some(line) = lines.get(ix).and_then(|l| l.strip_prefix("> ").or((*l == ">").then_some(""))) {
                    inner.push(line);
                    ix += 1;
                }
                let mut quote = Vec::new();
                self.lines(&inner.join("\n"), true, &mut quote);
                out.push(Block::Quote(quote));
                continue;
            }
            let heading = [("### ", 3), ("## ", 2), ("# ", 1)]
                .into_iter()
                .find_map(|(prefix, level)| Some((level, line.strip_prefix(prefix)?)));
            if let Some((level, text)) = heading {
                self.flush(&mut paragraph, out);
                out.push(Block::Text(TextKind::Heading(level), self.rich(text)));
            } else if let Some(text) = line.strip_prefix("-# ") {
                self.flush(&mut paragraph, out);
                out.push(Block::Text(TextKind::Subtext, self.rich(text)));
            } else if let Some((indent, number, text)) = list_item(line) {
                self.flush(&mut paragraph, out);
                let depth = (indent / 2).min(MAX_LIST_DEPTH);
                // Numbers count on from the item before at the same depth,
                // as markdown renumbers "1. 1. 1.".
                let marker = match number {
                    None => "•".to_owned(),
                    Some(number) => {
                        let previous = match out.last() {
                            Some(Block::Item { depth: d, marker, .. }) if *d == depth => {
                                marker.trim_end_matches('.').parse::<u32>().ok()
                            }
                            _ => None,
                        };
                        format!("{}.", previous.map_or(number, |p| p + 1))
                    }
                };
                out.push(Block::Item {
                    depth,
                    marker,
                    rich: self.rich(text),
                });
            } else {
                paragraph.push(line);
            }
            ix += 1;
        }
        self.flush(&mut paragraph, out);
    }

    /// Ends the paragraph being gathered, if any.
    fn flush(&mut self, paragraph: &mut Vec<&str>, out: &mut Vec<Block>) {
        if paragraph.is_empty() {
            return;
        }
        let text = paragraph.join("\n");
        paragraph.clear();
        out.push(Block::Text(TextKind::Paragraph, self.rich(&text)));
    }

    /// Lines of inline text in one [`Rich`].
    fn rich(&mut self, text: &str) -> Rich {
        let mut rich = Rich {
            text: String::with_capacity(text.len()),
            ..Rich::default()
        };
        for (ix, line) in text.split('\n').enumerate() {
            if ix > 0 {
                rich.text.push('\n');
            }
            self.inline(line, &mut rich);
        }
        rich
    }

    fn inline(&mut self, line: &str, rich: &mut Rich) {
        // Open markers with where their styled text starts in `rich.text`.
        let mut open: Vec<(&str, Style, usize)> = Vec::new();
        let mut rest = line;
        while let Some(c) = rest.chars().next() {
            if c == '\\' {
                // Escaped marker: keep the next character literally.
                let mut chars = rest.chars();
                chars.next();
                match chars.next() {
                    Some(next) if !next.is_alphanumeric() => {
                        rich.text.push(next);
                        rest = &rest[1 + next.len_utf8()..];
                    }
                    _ => {
                        rich.text.push('\\');
                        rest = &rest[1..];
                    }
                }
                continue;
            }
            if c == '`' {
                let fence = if rest.starts_with("```") { "```" } else { "`" };
                // Empty code is no code: "``" stays as typed.
                if let Some(end) = rest[fence.len()..].find(fence).filter(|end| *end > 0) {
                    let code = &rest[fence.len()..fence.len() + end];
                    let start = rich.text.len();
                    rich.text.push_str(code);
                    rich.spans.push((start..rich.text.len(), Style::Code));
                    rest = &rest[fence.len() * 2 + end..];
                    continue;
                }
            }
            if c == '[' {
                // Masked link: [label](url) or [label](<url>).
                if let Some((label, after)) = rest[1..].split_once("](") {
                    if let Some(end) = after.find(')') {
                        let url = after[..end].trim_start_matches('<').trim_end_matches('>');
                        if !label.contains('\n') && web_url(url).is_some() {
                            // A label can name any site, so the real host always
                            // follows it; no label check survives look-alike text.
                            rich.link(&format!("{label} ({})", host(url)), url);
                            rest = &after[end + 1..];
                            continue;
                        }
                    }
                }
            }
            if c == '<' {
                // <url> is a link Discord does not preview.
                if let Some(url) = bare_url(&rest[1..]).and_then(|u| u.split('>').next()) {
                    if rest[1 + url.len()..].starts_with('>') {
                        rich.link(url, url);
                        rest = &rest[url.len() + 2..];
                        continue;
                    }
                }
                if let Some((name, id, animated, len)) = custom_emoji(rest) {
                    if self.emojis_left > 0 {
                        self.emojis_left -= 1;
                        rich.emoji(id, animated);
                    } else {
                        rich.text.push_str(&format!(":{name}:"));
                    }
                    rest = &rest[len..];
                    continue;
                }
                if let Some((secs, style, len)) = timestamp(rest) {
                    let start = rich.text.len();
                    rich.text.push_str(&time::stamp(secs, style));
                    rich.spans.push((start..rich.text.len(), Style::Timestamp));
                    rest = &rest[len..];
                    continue;
                }
                if let Some((text, target, len)) = mention(rest, self.name) {
                    let start = rich.text.len();
                    rich.text.push_str(&text);
                    rich.spans.push((start..rich.text.len(), Style::Mention));
                    rich.mentions.push((start..rich.text.len(), target));
                    rest = &rest[len..];
                    continue;
                }
            }
            if c == '@' {
                if let Some(word) = ["@everyone", "@here"].into_iter().find(|w| rest.starts_with(w)) {
                    let start = rich.text.len();
                    rich.text.push_str(word);
                    rich.spans.push((start..rich.text.len(), Style::Mention));
                    rest = &rest[word.len()..];
                    continue;
                }
            }
            if let Some(url) = bare_url(rest) {
                match channel_link(url) {
                    // A pill naming the channel, as Discord draws these.
                    Some((_, channel, message)) => {
                        let name = (self.name)(Mention::Channel(channel));
                        let label = match message {
                            Some(_) => format!("#{name} › 💬"),
                            None => format!("#{name}"),
                        };
                        let start = rich.text.len();
                        rich.text.push_str(&label);
                        rich.spans.push((start..rich.text.len(), Style::Mention));
                        rich.links.push((start..rich.text.len(), url.to_owned()));
                    }
                    None => rich.link(url, url),
                }
                rest = &rest[url.len()..];
                continue;
            }
            if let Some(&(marker, style)) = MARKERS[1..].iter().find(|(m, _)| rest.starts_with(m)) {
                if let Some(ix) = open.iter().rposition(|(m, _, _)| *m == marker) {
                    let (_, _, start) = open.remove(ix);
                    if start < rich.text.len() {
                        rich.spans.push((start..rich.text.len(), style));
                        if style == Style::Spoiler {
                            rich.spoilers.push(start..rich.text.len());
                        }
                    }
                    rest = &rest[marker.len()..];
                    continue;
                }
                // Only open when a closing marker follows on this line.
                if rest[marker.len()..].contains(marker) {
                    open.push((marker, style, rich.text.len()));
                    rest = &rest[marker.len()..];
                    continue;
                }
            }
            rich.text.push(c);
            rest = &rest[c.len_utf8()..];
        }
        // Spoilers close in source order, which nesting can scramble.
        rich.spoilers.sort_by_key(|range| range.start);
    }
}

/// `<@id>`, `<@!id>`, `<#id>` and `<@&id>`, as the shown text, what it
/// names and the source length consumed.
fn mention(source: &str, name: &dyn Fn(Mention) -> String) -> Option<(String, Mention, usize)> {
    let end = source.find('>')?;
    let inner = &source[1..end];
    let id = |s: &str| s.parse().ok().map(Snowflake);
    let (target, sigil) = if let Some(rest) = inner.strip_prefix("@&") {
        (Mention::Role(id(rest)?), '@')
    } else if let Some(rest) = inner.strip_prefix('@') {
        (Mention::User(id(rest.trim_start_matches('!'))?), '@')
    } else {
        (Mention::Channel(id(inner.strip_prefix('#')?)?), '#')
    };
    Some((format!("{sigil}{}", name(target)), target, end + 1))
}

/// `<:name:id>` or `<a:name:id>`, as the name, id, whether animated and
/// the source length.
fn custom_emoji(source: &str) -> Option<(&str, Snowflake, bool, usize)> {
    let end = source.find('>')?;
    let inner = &source[1..end];
    let animated = inner.starts_with("a:");
    let rest = inner.strip_prefix("a:").or(inner.strip_prefix(':'))?;
    let (name, id) = rest.split_once(':')?;
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    Some((name, Snowflake(id.parse().ok()?), animated, end + 1))
}

/// `<t:secs>` or `<t:secs:style>`, as the time, its style (`f` when left
/// out) and the source length.
fn timestamp(source: &str) -> Option<(i64, char, usize)> {
    let end = source.find('>')?;
    let inner = source[1..end].strip_prefix("t:")?;
    let (secs, style) = match inner.split_once(':') {
        Some((secs, style)) => {
            let mut chars = style.chars();
            let style = chars.next().filter(|c| "tTdDfFR".contains(*c) && chars.next().is_none())?;
            (secs, style)
        }
        None => (inner, 'f'),
    };
    // Discord accepts these up to the year 275760; beyond, they are text.
    let secs = secs.parse::<i64>().ok().filter(|s| s.abs() <= 8_640_000_000_000)?;
    Some((secs, style, end + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(mention: Mention) -> String {
        match mention {
            Mention::User(id) => format!("user{id}"),
            Mention::Channel(id) => format!("chan{id}"),
            Mention::Role(id) => format!("role{id}"),
        }
    }

    fn spans(source: &str) -> (String, Vec<(String, Style)>) {
        let rich = parse(source, &names);
        let spans = rich
            .spans
            .iter()
            .map(|(range, style)| (rich.text[range.clone()].to_owned(), *style))
            .collect();
        (rich.text, spans)
    }

    /// Blocks as short strings, to compare whole structures at once.
    fn outline(source: &str) -> Vec<String> {
        fn walk(blocks: &[Block], prefix: &str, out: &mut Vec<String>) {
            for block in blocks {
                match block {
                    Block::Text(kind, rich) => out.push(format!("{prefix}{kind:?}:{}", rich.text)),
                    Block::Item { depth, marker, rich } => out.push(format!("{prefix}{depth}{marker}{}", rich.text)),
                    Block::Quote(inner) => walk(inner, &format!("{prefix}> "), out),
                    Block::Code(text) => out.push(format!("{prefix}code:{text}")),
                }
            }
        }
        let mut out = Vec::new();
        walk(&blocks(source, &names), "", &mut out);
        out
    }

    #[test]
    fn styles_and_unmatched_markers() {
        let (text, spans) = spans("**oi** *a* ~~b~~ __c__ 2*3");
        assert_eq!(text, "oi a b c 2*3");
        assert_eq!(
            spans,
            vec![
                ("oi".into(), Style::Bold),
                ("a".into(), Style::Italic),
                ("b".into(), Style::Strike),
                ("c".into(), Style::Underline),
            ]
        );
    }

    #[test]
    fn code_keeps_markers_literal() {
        let (text, spans) = spans("use `**x**` ok");
        assert_eq!(text, "use **x** ok");
        assert_eq!(spans, vec![("**x**".into(), Style::Code)]);
    }

    #[test]
    fn mentions_emoji_and_links() {
        let rich = parse("<@!5> <#6> <@&7> <:wave:8> <a:dance:9> https://x.io/a b", &names);
        assert_eq!(
            rich.text,
            format!("@user5 #chan6 @role7 {EMOJI_ROOM} {EMOJI_ROOM} https://x.io/a b")
        );
        let start = rich.text.find(EMOJI_ROOM).unwrap();
        assert_eq!(
            rich.emojis[0],
            CustomEmoji {
                range: start..start + EMOJI_ROOM.len(),
                id: Snowflake(8),
                animated: false
            }
        );
        assert!(rich.emojis[1].animated);
        let targets: Vec<_> = rich.mentions.iter().map(|(_, m)| *m).collect();
        assert_eq!(
            targets,
            [Mention::User(Snowflake(5)), Mention::Channel(Snowflake(6)), Mention::Role(Snowflake(7))]
        );
        let (_, spans) = spans("<@!5> <#6> <@&7> <:wave:8> https://x.io/a b");
        assert_eq!(spans[0], ("@user5".into(), Style::Mention));
        assert_eq!(spans[3], (EMOJI_ROOM.into(), Style::Hidden));
        assert_eq!(spans[4], ("https://x.io/a".into(), Style::Link));
    }

    #[test]
    fn emoji_only_messages_are_jumbo() {
        let jumbo = |source| parse(source, &names).jumbo();
        assert!(jumbo("<:a:1> <a:b:2>"));
        assert!(jumbo("👍 <:a:1> 👨‍👩‍👧 ❤️"));
        assert!(!jumbo("oi <:a:1>"));
        assert!(!jumbo(""));
        let many = "<:a:1>".repeat(31);
        assert!(!jumbo(&many));
        assert!(!jumbo("<:bad name:1>"));
    }

    #[test]
    fn emoji_images_are_capped_per_message() {
        let source: String = (0..40).map(|n| format!("<:e{n}:{n}>")).collect();
        let rich = parse(&source, &names);
        assert_eq!(rich.emojis.len(), MAX_EMOJI_IMAGES);
        assert!(rich.text.ends_with(":e39:"));
        assert_eq!(unicode_emoji("👨‍👩‍👧 👍🏽"), Some(2));
        // The cap holds across the blocks of one message.
        let half: String = (0..20).map(|n| format!("<:e{n}:{n}>")).collect();
        let images: usize = blocks(&format!("{half}\n> {half}"), &names)
            .iter()
            .map(|block| match block {
                Block::Text(_, rich) => rich.emojis.len(),
                Block::Quote(inner) => match &inner[0] {
                    Block::Text(_, rich) => rich.emojis.len(),
                    _ => 0,
                },
                _ => 0,
            })
            .sum();
        assert_eq!(images, MAX_EMOJI_IMAGES);
    }

    #[test]
    fn masked_and_unpreviewed_links() {
        let rich = parse("[Terms](<https://a.io/t>) e <https://b.io> ok", &names);
        assert_eq!(rich.text, "Terms (a.io) e https://b.io ok");
        let links: Vec<_> = rich
            .links
            .iter()
            .map(|(r, url)| (&rich.text[r.clone()], url.as_str()))
            .collect();
        assert_eq!(links, vec![("Terms (a.io)", "https://a.io/t"), ("https://b.io", "https://b.io")]);
    }

    #[test]
    fn masked_links_cannot_disguise_their_target() {
        let shown = |source| parse(source, &names).text;
        assert_eq!(shown("[discord.com](https://evil.io/x)"), "discord.com (evil.io)");
        assert_eq!(shown("[discord\u{200b}.com](https://a@evil.io)"), "discord\u{200b}.com (evil.io)");
        assert_eq!(web_url("javascript:alert(1)"), None);
    }

    #[test]
    fn headings_and_escapes() {
        let (text, spans) = spans("# Title\n\\*not\\*");
        assert_eq!(text, "Title\n*not*");
        assert_eq!(spans, vec![("Title".into(), Style::Bold)]);
    }

    #[test]
    fn channel_links_open_in_the_app() {
        let s = Snowflake;
        assert_eq!(channel_link("https://discord.com/channels/1/2/3"), Some((Some(s(1)), s(2), Some(s(3)))));
        assert_eq!(channel_link("https://ptb.discord.com/channels/@me/2"), Some((None, s(2), None)));
        assert_eq!(channel_link("https://discordapp.com/channels/1/2/"), Some((Some(s(1)), s(2), None)));
        assert_eq!(channel_link("https://evil.com/channels/1/2"), None);
        assert_eq!(channel_link("https://discord.com.evil.io/channels/1/2"), None);
        assert_eq!(channel_link("https://discord.com/channels/1/x"), None);
        assert_eq!(channel_link("https://discord.com/channels/1/2/3/4"), None);
        let rich = parse("see https://discord.com/channels/1/6/3 now", &names);
        assert_eq!(rich.text, "see #chan6 › 💬 now");
        assert_eq!(rich.links[0].1, "https://discord.com/channels/1/6/3");
    }

    #[test]
    fn spoilers_are_listed_in_order() {
        let rich = parse("a ||b|| c ||**d**||", &names);
        assert_eq!(rich.text, "a b c d");
        let hidden: Vec<_> = rich.spoilers.iter().map(|r| &rich.text[r.clone()]).collect();
        assert_eq!(hidden, ["b", "d"]);
    }

    #[test]
    fn timestamps_render_and_bad_ones_stay_text() {
        let rich = parse("at <t:0:R> and <t:x:R> <t:5:Q>", &names);
        assert!(rich.text.starts_with("at "));
        assert!(rich.text.ends_with(" and <t:x:R> <t:5:Q>"));
        assert_eq!(rich.spans[0].1, Style::Timestamp);
        assert_eq!(timestamp("<t:1700000000>"), Some((1_700_000_000, 'f', 14)));
    }

    #[test]
    fn quotes_lists_headings_and_subtext() {
        assert_eq!(
            outline("# Big\nplain\nmore\n> q1\n> q2\nafter\n-# small\n- a\n  - b\n1. x\n1. y\n>>> all\n# rest"),
            [
                "Heading(1):Big",
                "Paragraph:plain\nmore",
                "> Paragraph:q1\nq2",
                "Paragraph:after",
                "Subtext:small",
                "0•a",
                "1•b",
                "01.x",
                "02.y",
                "> Paragraph:all",
                "> Heading(1):rest",
            ]
        );
        // No space after the marker: plain text.
        assert_eq!(outline(">no\n-#no\n#no"), ["Paragraph:>no\n-#no\n#no"]);
    }

    #[test]
    fn fenced_code_blocks() {
        assert_eq!(
            outline("look:\n```rust\nfn x() {}\n```\nafter"),
            ["Paragraph:look:", "code:fn x() {}", "Paragraph:after"]
        );
        assert_eq!(outline("```a b```"), ["code:a b"]);
        assert_eq!(outline("```\n**raw**\n```"), ["code:**raw**"]);
        // Unclosed: literal text.
        assert_eq!(outline("```x"), ["Paragraph:```x"]);
    }
}
