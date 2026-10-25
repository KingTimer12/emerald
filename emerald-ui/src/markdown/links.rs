//! URLs in messages: bare links, which ones may open, and `discord.com/channels` links.

use super::*;

/// `https://…` or `http://…` up to whitespace.
pub(super) fn bare_url(source: &str) -> Option<&str> {
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
pub(super) fn host(url: &str) -> &str {
    let authority = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = &authority[..authority.find(['/', '?', '#']).unwrap_or(authority.len())];
    authority.rsplit('@').next().unwrap_or(authority)
}
