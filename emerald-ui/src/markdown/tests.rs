use super::inline::*;
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

#[test]
fn invite_links_are_found_once() {
    assert_eq!(invite_link("https://discord.gg/rust"), Some("rust"));
    assert_eq!(invite_link("https://discord.com/invite/abc-1?event=2"), Some("abc-1"));
    assert_eq!(invite_link("https://discord.com/channels/1/2"), None);
    assert_eq!(invite_link("https://evil.gg/rust"), None);
    let text = "join <https://discord.gg/a> or https://discord.gg/a and (https://discord.com/invite/b)";
    assert_eq!(invite_codes(text, 3), ["a", "b"]);
    assert_eq!(invite_codes(text, 1), ["a"]);
}
