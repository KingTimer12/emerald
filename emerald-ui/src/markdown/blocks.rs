//! Splitting a message into blocks: fenced code, quotes, lists, headings and paragraphs.

use super::*;

/// A fenced code block found in some text.
pub(super) struct Fence<'a> {
    /// Where the opening fence starts and past where the closing one ends.
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) code: &'a str,
}

/// The first ```` ``` ```` pair with something between. A first line of one
/// word names the language, when more lines follow it.
pub(super) fn code_fence(source: &str) -> Option<Fence<'_>> {
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
pub(super) fn list_item(line: &str) -> Option<(usize, Option<u32>, &str)> {
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
    pub(super) fn lines(&mut self, text: &str, quoted: bool, out: &mut Vec<Block>) {
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
    pub(super) fn flush(&mut self, paragraph: &mut Vec<&str>, out: &mut Vec<Block>) {
        if paragraph.is_empty() {
            return;
        }
        let text = paragraph.join("\n");
        paragraph.clear();
        out.push(Block::Text(TextKind::Paragraph, self.rich(&text)));
    }
}
