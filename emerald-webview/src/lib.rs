//! A native webview window that blocks until its page posts a message back,
//! then closes and returns it. Built for flows that need a real browser
//! engine (a sign-in page, a captcha) and hand back one string.
//!
//! ```no_run
//! use emerald_webview::{Page, WebView};
//!
//! let answer = WebView::new("Sign in", Page::Url("https://example.com/login"))
//!     .trust("example.com")
//!     .script(&format!("{}; emeraldPost('hi');", emerald_webview::POST_JS))
//!     .size(800., 600.)
//!     .show();
//! ```
//!
//! Each platform backend implements [`Backend`]: `WKWebView` on macOS,
//! WebView2 on Windows. Both pump their native event loop by hand, so the
//! window must open on the main thread of a process that runs no other UI
//! toolkit; apps with their own event loop run it in a child process.
//! Sessions are always ephemeral: nothing the page stores outlives the
//! window.

mod trust;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

use std::fmt;
use std::path::PathBuf;

pub use trust::is_trusted;

/// Defines `emeraldPost(value)` in the page, which sends a string to the
/// native side on every platform. Prepend it to scripts and pages that
/// post.
pub const POST_JS: &str = r#"
function emeraldPost(v) {
  if (window.webkit && window.webkit.messageHandlers) window.webkit.messageHandlers.emerald.postMessage(v);
  else if (window.chrome && window.chrome.webview) window.chrome.webview.postMessage(v);
}
"#;

/// What the window shows.
#[derive(Clone, Copy, Debug)]
pub enum Page<'a> {
    /// A remote page.
    Url(&'a str),
    /// Local HTML served as if it came from `base`, so it gets that
    /// origin (some widgets only render on their own site).
    Html { html: &'a str, base: &'a str },
}

/// One window to show, built step by step.
#[derive(Clone, Debug)]
pub struct WebView<'a> {
    pub title: &'a str,
    pub page: Page<'a>,
    /// Runs at document start in the main frame of every page.
    pub script: Option<&'a str>,
    /// Content size in points.
    pub size: (f64, f64),
    /// Only the main frame of `https://{trusted_host}` may post back.
    pub trusted_host: &'a str,
    /// Where the browser engine keeps its profile, on platforms that need
    /// one (WebView2). `None` lets the engine choose.
    pub data_dir: Option<PathBuf>,
}

impl<'a> WebView<'a> {
    pub fn new(title: &'a str, page: Page<'a>) -> Self {
        Self {
            title,
            page,
            script: None,
            size: (800., 600.),
            trusted_host: "",
            data_dir: None,
        }
    }

    pub fn script(mut self, source: &'a str) -> Self {
        self.script = Some(source);
        self
    }

    pub fn size(mut self, width: f64, height: f64) -> Self {
        self.size = (width, height);
        self
    }

    pub fn trust(mut self, host: &'a str) -> Self {
        self.trusted_host = host;
        self
    }

    pub fn data_dir(mut self, dir: Option<PathBuf>) -> Self {
        self.data_dir = dir;
        self
    }

    /// Opens the window and blocks until the page posts a non-empty
    /// message (`Ok(Some)`) or the user closes it (`Ok(None)`).
    pub fn show(&self) -> Result<Option<String>, Error> {
        Platform::show(self)
    }
}

/// A platform's webview.
pub trait Backend {
    fn show(view: &WebView<'_>) -> Result<Option<String>, Error>;
}

#[cfg(target_os = "macos")]
type Platform = macos::WkWebView;
#[cfg(windows)]
type Platform = windows::WebView2;
#[cfg(not(any(target_os = "macos", windows)))]
type Platform = Unsupported;

/// Platforms with no backend yet.
#[cfg(not(any(target_os = "macos", windows)))]
pub struct Unsupported;

#[cfg(not(any(target_os = "macos", windows)))]
impl Backend for Unsupported {
    fn show(_: &WebView<'_>) -> Result<Option<String>, Error> {
        Err(Error("no webview on this platform".into()))
    }
}

/// The webview could not open: the engine is missing or failed to start.
#[derive(Debug)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}
