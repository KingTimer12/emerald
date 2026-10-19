//! The subset of Discord markdown a message line can show with styled text:
//! bold, italic, underline, strikethrough, inline and block code, spoilers,
//! headings, links, and mentions resolved to names. Everything else stays
//! literal, as Discord leaves unmatched markers.

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
}

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
}

impl Rich {
    fn link(&mut self, label: &str, url: &str) {
        let start = self.text.len();
        self.text.push_str(label);
        self.spans.push((start..self.text.len(), Style::Link));
        self.links.push((start..self.text.len(), url.to_owned()));
    }
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

/// A masked link label that names a site, like `discord.com` or a URL. Such
/// a label could point somewhere else, so the real URL is shown instead, as
/// Discord refuses these labels.
fn looks_like_url(label: &str) -> bool {
    let label = label.trim();
    bare_url(label).is_some()
        || (!label.contains(char::is_whitespace)
            && label.split('/').next().is_some_and(|host| {
                host.contains('.') && host.split('.').all(|part| !part.is_empty())
            }))
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
                        let shown = if looks_like_url(label) { url } else { label };
                        rich.link(shown, url);
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
            if let Some((text, len, styled)) = mention(rest, name) {
                let start = rich.text.len();
                rich.text.push_str(&text);
                if styled {
                    rich.spans.push((start..rich.text.len(), Style::Mention));
                }
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

/// `<@id>`, `<@!id>`, `<#id>`, `<@&id>`, `<:name:id>` and `<a:name:id>`,
/// as the shown text, the source length consumed, and whether it is a
/// mention (styled) rather than an emoji (plain).
fn mention(source: &str, name: &dyn Fn(Mention) -> String) -> Option<(String, usize, bool)> {
    let end = source.find('>')?;
    let inner = &source[1..end];
    let id = |s: &str| s.parse().ok().map(Snowflake);
    let (text, styled) = if let Some(rest) = inner.strip_prefix("@&") {
        (format!("@{}", name(Mention::Role(id(rest)?))), true)
    } else if let Some(rest) = inner.strip_prefix('@') {
        (format!("@{}", name(Mention::User(id(rest.trim_start_matches('!'))?))), true)
    } else if let Some(rest) = inner.strip_prefix('#') {
        (format!("#{}", name(Mention::Channel(id(rest)?))), true)
    } else {
        // Custom emoji: shown by name, since inline images are not drawn.
        let rest = inner.strip_prefix("a:").or(inner.strip_prefix(':'))?;
        let (emoji, emoji_id) = rest.split_once(':')?;
        id(emoji_id)?;
        (format!(":{emoji}:"), false)
    };
    Some((text, end + 1, styled))
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
        let (text, spans) = spans("<@!5> <#6> <@&7> <:wave:8> https://x.io/a b");
        assert_eq!(text, "@user5 #chan6 @role7 :wave: https://x.io/a b");
        assert_eq!(spans[0], ("@user5".into(), Style::Mention));
        assert_eq!(spans[3], ("https://x.io/a".into(), Style::Link));
    }

    #[test]
    fn masked_and_unpreviewed_links() {
        let rich = parse("[Terms](<https://a.io/t>) e <https://b.io> ok", &names);
        assert_eq!(rich.text, "Terms e https://b.io ok");
        let links: Vec<_> = rich
            .links
            .iter()
            .map(|(r, url)| (&rich.text[r.clone()], url.as_str()))
            .collect();
        assert_eq!(links, vec![("Terms", "https://a.io/t"), ("https://b.io", "https://b.io")]);
    }

    #[test]
    fn masked_links_cannot_disguise_their_target() {
        for source in ["[discord.com](https://evil.io)", "[https://discord.com](https://evil.io)"] {
            assert_eq!(parse(source, &names).text, "https://evil.io");
        }
        assert_eq!(parse("[v1.2 notes](https://a.io)", &names).text, "v1.2 notes");
        assert_eq!(web_url("javascript:alert(1)"), None);
    }

    #[test]
    fn headings_and_escapes() {
        let (text, spans) = spans("# Título\n\\*não\\*");
        assert_eq!(text, "Título\n*não*");
        assert_eq!(spans, vec![("Título".into(), Style::Bold)]);
    }
}
