//! Where a family of requests goes, written once: a [`Client`](crate::Client)
//! joins it with each request's path, and URLs built elsewhere (CDN links,
//! the gateway) come from the same constants instead of repeating the host.

use std::fmt::{self, Display};

/// A scheme, host and optional path prefix, without a trailing slash.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BaseUrl(&'static str);

impl BaseUrl {
    /// `url` must not end in `/`; paths joined to it start with one.
    pub const fn new(url: &'static str) -> Self {
        let bytes = url.as_bytes();
        assert!(!bytes.is_empty() && bytes[bytes.len() - 1] != b'/', "a base URL has no trailing slash");
        Self(url)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }

    /// The full URL of `path`, which starts with `/` (or `?` for a query
    /// on the base itself).
    pub fn join(self, path: impl Display) -> String {
        format!("{}{path}", self.0)
    }
}

impl Display for BaseUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_paths() {
        const API: BaseUrl = BaseUrl::new("https://example.com/api/v9");
        assert_eq!(API.join("/users/@me"), "https://example.com/api/v9/users/@me");
        assert_eq!(API.join(format_args!("/channels/{}", 7)), "https://example.com/api/v9/channels/7");
    }
}
