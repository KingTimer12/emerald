//! The two flows that need Discord's own web stack, each in an
//! [`emerald_webview`] window: the official login and hCaptcha challenges.
//!
//! The webview owns the process' native event loop, and gpui insists on
//! owning it too, so each flow runs in a child invocation of this binary
//! (`--login`, `--captcha`) that prints its result to stdout.
//!
//! Token capture does not touch `localStorage` (Discord removed the token from
//! it). Instead an injected user script (`assets/capture.js`) wraps `fetch`
//! and `XMLHttpRequest` and forwards any `Authorization` header to native code
//! through the webview's message channel. That header is the raw account
//! token the web client sends on every `/api` request.

use emerald_discord::urls::WEB;
use emerald_webview::{POST_JS, Page, WebView};

/// Posts the `Authorization` header of the web client's first API request.
const CAPTURE_JS: &str = include_str!("../assets/capture.js");

/// hCaptcha widget page. Discord's sitekey only renders on discord.com, so
/// the page is served under that origin; the challenge itself is solved by
/// the user and the token goes back through the same message channel.
const CAPTCHA_HTML: &str = include_str!("../assets/captcha.html");

/// Only the main frame of this host may post results back.
const TRUSTED_HOST: &str = "discord.com";

/// Opens the login window and blocks until the user authenticates or closes it.
/// Returns the captured account token, or `None` if the window was dismissed.
pub fn acquire_token() -> Option<String> {
    let url = WEB.join("/login");
    let script = format!("{POST_JS}{CAPTURE_JS}");
    window(t!("login.title"), Page::Url(&url)).script(&script).size(960., 760.).show().unwrap_or_else(failed)
}

/// Shows the hCaptcha widget for a challenge and returns the token the user
/// produced by solving it, or `None` if the window was closed.
pub fn solve_captcha(sitekey: &str, rqdata: Option<&str>) -> Option<String> {
    // JSON string literals are valid JS literals, which keeps server-provided
    // values from breaking out of the script.
    let html = CAPTCHA_HTML
        .replace("__POST_JS__", POST_JS)
        .replace("__PROMPT__", &escape_html(t!("login.captcha_prompt")))
        .replace("__SITEKEY__", &serde_json::to_string(sitekey).ok()?)
        .replace("__RQDATA__", &serde_json::to_string(&rqdata).ok()?);
    let base = WEB.join("/");
    window(t!("login.captcha_title"), Page::Html { html: &html, base: &base })
        .size(420., 640.)
        .show()
        .unwrap_or_else(failed)
}

/// A window that trusts only discord.com, with its WebView2 profile kept
/// with the cache.
fn window<'a>(title: &'a str, page: Page<'a>) -> WebView<'a> {
    let data_dir = emerald_core::paths::cache_dir().map(|dir| dir.join("WebView2"));
    WebView::new(title, page).trust(TRUSTED_HOST).data_dir(data_dir)
}

fn failed(error: emerald_webview::Error) -> Option<String> {
    tracing::error!("{error}");
    None
}

/// Text as HTML element content.
fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}
