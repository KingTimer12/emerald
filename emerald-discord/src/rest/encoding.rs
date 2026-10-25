//! Percent-encoding the way the web client's URLs come out of the browser.

/// Percent-encodes a path segment the way the browser's URL parser does
/// when the web client puts raw text in a path: non-ASCII and the path
/// percent-encode set, while `:` stays as is.
pub fn encode_path(text: &str) -> String {
    percent_encode(text, |byte| byte.is_ascii_graphic() && !b"\"#<>?`{}/%".contains(&byte))
}

/// Percent-encodes a query value as `encodeURIComponent` does, which is
/// how the web client's HTTP library writes query strings.
pub fn encode_component(text: &str) -> String {
    percent_encode(text, |byte| byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte))
}

/// `text` with every byte `keep` refuses written as `%XX`.
fn percent_encode(text: &str, keep: impl Fn(u8) -> bool) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if keep(byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reaction_routes_encode_like_the_browser() {
        assert_eq!(encode_path("👍"), "%F0%9F%91%8D");
        assert_eq!(encode_path("pepe:42"), "pepe:42");
        assert_eq!(encode_component("Message Hover Bar"), "Message%20Hover%20Bar");
    }
}
