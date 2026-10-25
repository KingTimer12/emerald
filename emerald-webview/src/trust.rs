//! Which pages may post back: the main frame of one HTTPS host.

/// `https://{host}/...` only: no other scheme, host, port or userinfo.
pub fn is_trusted(source: &str, host: &str) -> bool {
    !host.is_empty()
        && source
            .strip_prefix("https://")
            .and_then(|rest| rest.split(['/', '?', '#']).next())
            .is_some_and(|authority| authority == host)
}

#[cfg(test)]
mod tests {
    use super::is_trusted;

    #[test]
    fn only_the_https_host_is_trusted() {
        let host = "discord.com";
        assert!(is_trusted("https://discord.com/login", host));
        assert!(is_trusted("https://discord.com/", host));
        assert!(is_trusted("https://discord.com?x=1", host));
        assert!(!is_trusted("http://discord.com/login", host));
        assert!(!is_trusted("https://discord.com.evil.io/", host));
        assert!(!is_trusted("https://discord.com@evil.io/", host));
        assert!(!is_trusted("https://evil.io/discord.com", host));
        assert!(!is_trusted("https://discord.com:8443/", host));
    }

    #[test]
    fn no_host_trusts_nothing() {
        assert!(!is_trusted("https://discord.com/", ""));
    }
}
