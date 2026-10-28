//! The release feed: one JSON file published with every release, naming
//! the version and, for each platform, the file to download and its
//! signature.
//!
//! ```json
//! {
//!   "version": "0.2.0",
//!   "notes": "…",
//!   "platforms": {
//!     "macos-aarch64": { "url": "https://…/Emerald-0.2.0-macos-aarch64.app.tar.gz", "signature": "…" },
//!     "windows-x86_64": { "url": "https://…/Emerald-0.2.0-setup-x86_64.exe", "signature": "…" }
//!   }
//! }
//! ```

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Feed {
    pub version: String,
    #[serde(default)]
    pub notes: String,
    pub platforms: BTreeMap<String, Artifact>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Artifact {
    pub url: String,
    /// The release key's ed25519 signature, in base64.
    pub signature: String,
}

/// This build's key in the feed's `platforms`: `macos-aarch64`,
/// `windows-x86_64` and so on.
pub fn platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

impl Feed {
    /// What to download when the feed offers something newer than
    /// `current` for this platform. Only https downloads are taken.
    pub fn newer(&self, current: &str, platform: &str) -> Option<&Artifact> {
        if version(&self.version)? <= version(current)? {
            return None;
        }
        self.platforms.get(platform).filter(|artifact| artifact.url.starts_with("https://"))
    }
}

/// `1.2.3` (a leading `v` and a `-beta` suffix are allowed) as numbers
/// that compare in order.
pub fn version(text: &str) -> Option<(u64, u64, u64)> {
    let text = text.trim().trim_start_matches('v');
    let text = text.split(['-', '+']).next()?;
    let mut parts = text.split('.').map(|part| part.parse::<u64>().ok());
    let version = (parts.next()??, parts.next().unwrap_or(Some(0))?, parts.next().unwrap_or(Some(0))?);
    parts.next().is_none().then_some(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(version: &str, url: &str) -> Feed {
        serde_json::from_value(serde_json::json!({
            "version": version,
            "platforms": { "macos-aarch64": { "url": url, "signature": "c2ln" } },
        }))
        .unwrap()
    }

    #[test]
    fn versions_compare_as_numbers() {
        assert!(version("0.10.0") > version("0.9.9"));
        assert_eq!(version("v1.2"), Some((1, 2, 0)));
        assert_eq!(version("1.2.3-beta.1"), Some((1, 2, 3)));
        assert_eq!(version("one"), None);
        assert_eq!(version("1.2.3.4"), None);
    }

    #[test]
    fn only_a_newer_https_build_for_this_platform_is_taken() {
        let url = "https://example.com/a.tar.gz";
        assert!(feed("0.2.0", url).newer("0.1.0", "macos-aarch64").is_some());
        assert!(feed("0.1.0", url).newer("0.1.0", "macos-aarch64").is_none());
        assert!(feed("0.2.0", url).newer("0.1.0", "windows-x86_64").is_none());
        assert!(feed("0.2.0", "http://example.com/a").newer("0.1.0", "macos-aarch64").is_none());
    }
}
