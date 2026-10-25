//! The search box's text read as Discord's filters: `from:`, `mentions:`,
//! `in:`, `has:`, `before:`, `during:`, `after:` and `pinned:`, the rest
//! being the words to find. Names are resolved to ids by the caller.

use emerald_discord::model::{DISCORD_EPOCH_MS, Snowflake, iso_secs};

/// What `has:` accepts, as Discord's search does.
pub const HAS: [&str; 9] = ["link", "embed", "file", "video", "image", "sound", "sticker", "poll", "forward"];

const DAY: i64 = 86_400;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filters {
    pub content: String,
    /// Names as typed, for the caller to find.
    pub from: Vec<String>,
    pub mentions: Vec<String>,
    pub channels: Vec<String>,
    pub has: Vec<String>,
    /// Bounds as snowflakes, from the dates.
    pub min_id: Option<Snowflake>,
    pub max_id: Option<Snowflake>,
    pub pinned: Option<bool>,
}

/// A filter that could not be read: the word, for the message.
#[derive(Debug, PartialEq, Eq)]
pub struct Bad(pub String);

/// Reads `text`. Dates are `YYYY-MM-DD`, taken in UTC: `before:` ends at
/// the start of that day, `after:` begins after it, `during:` is the day.
pub fn parse(text: &str) -> Result<Filters, Bad> {
    let mut filters = Filters::default();
    let mut words = Vec::new();
    let mut tokens = text.split_whitespace().peekable();
    while let Some(token) = tokens.next() {
        let Some((key, value)) = token.split_once(':').filter(|(key, _)| is_filter(key)) else {
            words.push(token);
            continue;
        };
        // `from: ana` is as good as `from:ana`.
        let value = match value {
            "" => tokens.next().ok_or_else(|| Bad(token.to_owned()))?,
            value => value,
        };
        let bad = || Bad(format!("{key}:{value}"));
        match key.to_lowercase().as_str() {
            "from" => filters.from.push(value.trim_start_matches('@').to_owned()),
            "mentions" => filters.mentions.push(value.trim_start_matches('@').to_owned()),
            "in" => filters.channels.push(value.trim_start_matches('#').to_owned()),
            "has" => {
                let value = value.to_lowercase();
                if !HAS.contains(&value.as_str()) {
                    return Err(bad());
                }
                filters.has.push(value);
            }
            "pinned" => filters.pinned = Some(value.parse().map_err(|_| bad())?),
            "before" => filters.max_id = Some(at(day(value).ok_or_else(bad)?)),
            "after" => filters.min_id = Some(at(day(value).ok_or_else(bad)? + DAY)),
            "during" => {
                let day = day(value).ok_or_else(bad)?;
                filters.min_id = Some(at(day));
                filters.max_id = Some(at(day + DAY));
            }
            _ => unreachable!("checked by is_filter"),
        }
    }
    filters.content = words.join(" ");
    Ok(filters)
}

fn is_filter(key: &str) -> bool {
    ["from", "mentions", "in", "has", "pinned", "before", "after", "during"].contains(&key.to_lowercase().as_str())
}

/// The start of `YYYY-MM-DD`, in unix seconds.
fn day(text: &str) -> Option<i64> {
    let mut parts = text.split('-');
    let valid = parts.clone().count() == 3 && parts.all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    valid.then(|| iso_secs(&format!("{text}T00:00:00"))).flatten()
}

/// The smallest snowflake at `secs`, which is how search bounds dates.
pub fn at(secs: i64) -> Snowflake {
    let ms = (secs * 1000 - DISCORD_EPOCH_MS as i64).max(0) as u64;
    Snowflake(ms << 22)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_come_out_of_the_words() {
        let filters = parse("from:@ana has:image  bom dia in:#geral mentions: bruno pinned:true").unwrap();
        assert_eq!(filters.content, "bom dia");
        assert_eq!(filters.from, ["ana"]);
        assert_eq!(filters.mentions, ["bruno"]);
        assert_eq!(filters.channels, ["geral"]);
        assert_eq!(filters.has, ["image"]);
        assert_eq!(filters.pinned, Some(true));
        // A colon in plain words is only text.
        assert_eq!(parse("hora: 10:30").unwrap().content, "hora: 10:30");
    }

    #[test]
    fn dates_bound_by_snowflake() {
        let filters = parse("during:2016-04-30").unwrap();
        let (min, max) = (filters.min_id.unwrap(), filters.max_id.unwrap());
        assert_eq!(min.unix_secs(), iso_secs("2016-04-30T00:00:00").unwrap());
        assert_eq!(max.unix_secs() - min.unix_secs(), DAY);
        assert_eq!(parse("after:2016-04-30").unwrap().min_id, Some(max));
        assert_eq!(parse("before:2016-04-30").unwrap().max_id, Some(min));
        assert_eq!(parse("before:yesterday"), Err(Bad("before:yesterday".into())));
        assert_eq!(parse("has:cats"), Err(Bad("has:cats".into())));
        assert_eq!(parse("from:"), Err(Bad("from:".into())));
    }
}
