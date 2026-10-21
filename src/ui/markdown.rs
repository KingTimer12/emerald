//! The subset of Discord markdown a message line can show with styled text:
//! bold, italic, underline, strikethrough, inline and block code, spoilers,
//! headings, links, mentions resolved to names, and custom emoji. Everything
//! else stays literal, as Discord leaves unmatched markers.

use std::ops::Range;

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

/// What a `<…>` token refers to, for the caller to name.
pub enum Mention {
    User(Snowflake),
    Channel(Snowflake),
    Role(Snowflake),
}

pub struct Rich {
    pub text: String,
    pub spans: Vec<(Range<usize>, Style)>,
    /// Clickable ranges of `text` with the URL each opens.
    pub links: Vec<(Range<usize>, String)>,
    /// Custom emoji: the room kept for each in `text`, and its id.
    pub emojis: Vec<(Range<usize>, Snowflake)>,
}

impl Rich {
    fn link(&mut self, label: &str, url: &str) {
        let start = self.text.len();
        self.text.push_str(label);
        self.spans.push((start..self.text.len(), Style::Link));
        self.links.push((start..self.text.len(), url.to_owned()));
    }

    fn emoji(&mut self, id: Snowflake) {
        let start = self.text.len();
        self.text.push_str(EMOJI_ROOM);
        self.spans.push((start..self.text.len(), Style::Hidden));
        self.emojis.push((start..self.text.len(), id));
    }

    /// Whether the text is nothing but emoji, few enough to show large.
    pub fn jumbo(&self) -> bool {
        let mut count = self.emojis.len();
        let mut rest = self.text.as_str();
        let mut last = 0;
        for (range, _) in &self.emojis {
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

pub fn parse(source: &str, name: &dyn Fn(Mention) -> String) -> Rich {
    let mut rich = Rich {
        text: String::with_capacity(source.len()),
        spans: Vec::new(),
        links: Vec::new(),
        emojis: Vec::new(),
    };
    for (ix, line) in source.split('\n').enumerate() {
        if ix > 0 {
            rich.text.push('\n');
        }
        // "# Title" lines render bold without the hashes.
        let heading = ["### ", "## ", "# "].iter().find_map(|h| line.strip_prefix(h));
        let start = rich.text.len();
        inline(heading.unwrap_or(line), name, &mut rich);
        if heading.is_some() {
            rich.spans.push((start..rich.text.len(), Style::Bold));
        }
    }
    rich
}

fn inline(line: &str, name: &dyn Fn(Mention) -> String, rich: &mut Rich) {
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
            if let Some(end) = rest[fence.len()..].find(fence) {
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
            if let Some((name, id, len)) = custom_emoji(rest) {
                if rich.emojis.len() < MAX_EMOJI_IMAGES {
                    rich.emoji(id);
                } else {
                    rich.text.push_str(&format!(":{name}:"));
                }
                rest = &rest[len..];
                continue;
            }
            if let Some((text, len)) = mention(rest, name) {
                let start = rich.text.len();
                rich.text.push_str(&text);
                rich.spans.push((start..rich.text.len(), Style::Mention));
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
            rich.link(url, url);
            rest = &rest[url.len()..];
            continue;
        }
        if let Some(&(marker, style)) = MARKERS[1..].iter().find(|(m, _)| rest.starts_with(m)) {
            if let Some(ix) = open.iter().rposition(|(m, _, _)| *m == marker) {
                let (_, _, start) = open.remove(ix);
                if start < rich.text.len() {
                    rich.spans.push((start..rich.text.len(), style));
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
}

/// `<@id>`, `<@!id>`, `<#id>` and `<@&id>`, as the shown text and the
/// source length consumed.
fn mention(source: &str, name: &dyn Fn(Mention) -> String) -> Option<(String, usize)> {
    let end = source.find('>')?;
    let inner = &source[1..end];
    let id = |s: &str| s.parse().ok().map(Snowflake);
    let text = if let Some(rest) = inner.strip_prefix("@&") {
        format!("@{}", name(Mention::Role(id(rest)?)))
    } else if let Some(rest) = inner.strip_prefix('@') {
        format!("@{}", name(Mention::User(id(rest.trim_start_matches('!'))?)))
    } else {
        format!("#{}", name(Mention::Channel(id(inner.strip_prefix('#')?)?)))
    };
    Some((text, end + 1))
}

/// `<:name:id>` or `<a:name:id>`, as the name, id and source length.
fn custom_emoji(source: &str) -> Option<(&str, Snowflake, usize)> {
    let end = source.find('>')?;
    let inner = &source[1..end];
    let rest = inner.strip_prefix("a:").or(inner.strip_prefix(':'))?;
    let (name, id) = rest.split_once(':')?;
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    Some((name, Snowflake(id.parse().ok()?), end + 1))
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
        let rich = parse("<@!5> <#6> <@&7> <:wave:8> https://x.io/a b", &names);
        assert_eq!(rich.text, format!("@user5 #chan6 @role7 {EMOJI_ROOM} https://x.io/a b"));
        let start = rich.text.find(EMOJI_ROOM).unwrap();
        assert_eq!(rich.emojis, vec![(start..start + EMOJI_ROOM.len(), Snowflake(8))]);
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
        let (text, spans) = spans("# Título\n\\*não\\*");
        assert_eq!(text, "Título\n*não*");
        assert_eq!(spans, vec![("Título".into(), Style::Bold)]);
    }
}
