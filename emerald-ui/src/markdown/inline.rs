//! Inline spans: style markers, mentions, custom emoji and timestamps.

use super::*;

/// Paired markers, longest first so `**` wins over `*`.
pub(super) const MARKERS: [(&str, Style); 7] = [
    ("```", Style::Code),
    ("||", Style::Spoiler),
    ("**", Style::Bold),
    ("__", Style::Underline),
    ("~~", Style::Strike),
    ("*", Style::Italic),
    ("_", Style::Italic),
];

impl Parser<'_> {
    /// Lines of inline text in one [`Rich`].
    pub(super) fn rich(&mut self, text: &str) -> Rich {
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

    pub(super) fn inline(&mut self, line: &str, rich: &mut Rich) {
        // Open markers with where their styled text starts in `rich.text`.
        let mut open: Vec<(&str, Style, usize)> = Vec::new();
        let mut rest = line;
        while let Some(c) = rest.chars().next() {
            // Plain text up to the next character that can start something
            // goes in at once.
            if !starts_token(c) {
                let end = rest.find(starts_token).unwrap_or(rest.len());
                rich.text.push_str(&rest[..end]);
                rest = &rest[end..];
                continue;
            }
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
            let token = match c {
                '[' => self.masked_link(rest, rich),
                '<' => self.angle_token(rest, rich),
                '@' => everyone(rest, rich),
                _ => None,
            };
            if let Some(len) = token {
                rest = &rest[len..];
                continue;
            }
            if let Some(url) = bare_url(rest) {
                match channel_link(url) {
                    // A pill naming the channel, as Discord draws these.
                    Some((_, channel, message)) => {
                        let start = rich.text.len();
                        rich.text.push('#');
                        rich.text.push_str(&(self.name)(Mention::Channel(channel)));
                        if message.is_some() {
                            rich.text.push_str(" › 💬");
                        }
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

    /// `[label](url)` or `[label](<url>)`: the link, and the source length.
    pub(super) fn masked_link(&mut self, rest: &str, rich: &mut Rich) -> Option<usize> {
        let (label, after) = rest[1..].split_once("](")?;
        let end = after.find(')')?;
        let url = after[..end].trim_start_matches('<').trim_end_matches('>');
        if label.contains('\n') || web_url(url).is_none() {
            return None;
        }
        // A label can name any site, so the real host always follows it; no
        // label check survives look-alike text.
        rich.link(&format!("{label} ({})", host(url)), url);
        Some(rest.len() - after[end + 1..].len())
    }

    /// What starts with `<`: an unpreviewed `<url>`, a custom emoji, a
    /// timestamp or a mention.
    pub(super) fn angle_token(&mut self, rest: &str, rich: &mut Rich) -> Option<usize> {
        if let Some(url) = bare_url(&rest[1..]).and_then(|u| u.split('>').next())
            && rest[1 + url.len()..].starts_with('>')
        {
            rich.link(url, url);
            return Some(url.len() + 2);
        }
        if let Some((name, id, animated, len)) = custom_emoji(rest) {
            if self.emojis_left > 0 {
                self.emojis_left -= 1;
                rich.emoji(id, animated);
            } else {
                rich.text.push(':');
                rich.text.push_str(name);
                rich.text.push(':');
            }
            return Some(len);
        }
        if let Some((secs, style, len)) = timestamp(rest) {
            rich.push_styled(&time::stamp(secs, style), Style::Timestamp);
            return Some(len);
        }
        let (text, target, len) = mention(rest, self.name)?;
        let range = rich.push_styled(&text, Style::Mention);
        rich.mentions.push((range, target));
        Some(len)
    }
}

/// Whether `c` can begin an escape, code, link, mention, emoji, URL or
/// style marker; anything else is plain text.
fn starts_token(c: char) -> bool {
    matches!(c, '\\' | '`' | '[' | '<' | '@' | 'h' | '|' | '*' | '_' | '~')
}

/// `<@id>`, `<@!id>`, `<#id>` and `<@&id>`, as the shown text, what it
/// names and the source length consumed.
pub(super) fn mention(source: &str, name: &dyn Fn(Mention) -> String) -> Option<(String, Mention, usize)> {
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
pub(super) fn custom_emoji(source: &str) -> Option<(&str, Snowflake, bool, usize)> {
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
pub(super) fn timestamp(source: &str) -> Option<(i64, char, usize)> {
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

/// `@everyone` or `@here`, styled as mentions.
pub(super) fn everyone(rest: &str, rich: &mut Rich) -> Option<usize> {
    let word = ["@everyone", "@here"].into_iter().find(|w| rest.starts_with(w))?;
    rich.push_styled(word, Style::Mention);
    Some(word.len())
}
