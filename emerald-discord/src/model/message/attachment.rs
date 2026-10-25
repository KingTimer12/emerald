//! Files attached to a message.

use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct Attachment {
    pub filename: String,
    /// The file on the CDN, to open in the browser.
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub proxy_url: Option<String>,
    /// In bytes.
    #[serde(default)]
    pub size: u64,
    /// Voice messages and other audio: their length.
    #[serde(default)]
    pub duration_secs: Option<f64>,
    #[serde(default)]
    pub content_type: Option<String>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    /// Alt text.
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub flags: u64,
}

impl Attachment {
    /// Sent as a spoiler: flagged, or named the way older clients mark it.
    pub fn spoiler(&self) -> bool {
        const IS_SPOILER: u64 = 1 << 3;
        self.flags & IS_SPOILER != 0 || self.filename.starts_with("SPOILER_")
    }
}
