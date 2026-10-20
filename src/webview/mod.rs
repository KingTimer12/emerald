//! Native webview windows for the two flows that need Discord's own web
//! stack: the official login and hCaptcha challenges. macOS uses `WKWebView`,
//! Windows uses WebView2.
//!
//! The webview owns the process' native event loop, and gpui insists on
//! owning it too, so each flow runs in a child invocation of this binary
//! (`--login`, `--captcha`) that prints its result to stdout. Inside the
//! child, events are pumped manually until the flow finishes.
//!
//! Token capture does not touch `localStorage` (Discord removed the token from
//! it). Instead an injected user script wraps `fetch` and `XMLHttpRequest` and
//! forwards any `Authorization` header to native code through the webview's
//! message channel. That header is the raw account token the web client sends
//! on every `/api` request.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as platform;

#[cfg(windows)]
mod webview2;
#[cfg(windows)]
use webview2 as platform;

use std::io::Write;

/// Posts a string to native code: `WKScriptMessageHandler` named `emerald`
/// on macOS, `chrome.webview` on Windows. Prepended to every page script.
const POST_JS: &str = r#"
function emeraldPost(v) {
  if (window.webkit && window.webkit.messageHandlers) window.webkit.messageHandlers.emerald.postMessage(v);
  else if (window.chrome && window.chrome.webview) window.chrome.webview.postMessage(v);
}
"#;

/// Injected at document start. Wraps the two ways the web client sets request
/// headers and posts the `Authorization` value back exactly once.
const CAPTURE_JS: &str = r#"
(function () {
  // Only the top document; iframes are never trusted with the token.
  if (window.top !== window) return;
  let sent = false;
  function report(v) {
    if (sent || !v) return;
    v = String(v).trim();
    if (!v || v.toLowerCase().startsWith('bearer ')) return;
    sent = true;
    try { emeraldPost(v); } catch (e) {}
  }
  const of = window.fetch;
  window.fetch = function (input, init) {
    try {
      const h = init && init.headers;
      if (h) {
        if (typeof h.get === 'function') report(h.get('Authorization'));
        else report(h['Authorization'] || h['authorization']);
      }
    } catch (e) {}
    return of.apply(this, arguments);
  };
  const os = XMLHttpRequest.prototype.setRequestHeader;
  XMLHttpRequest.prototype.setRequestHeader = function (k, v) {
    try { if (String(k).toLowerCase() === 'authorization') report(v); } catch (e) {}
    return os.apply(this, arguments);
  };
})();
"#;

/// hCaptcha widget page. Discord's sitekey only renders on discord.com, so
/// the page is served under that origin; the challenge itself is solved by
/// the user and the token goes back through the same message channel.
const CAPTCHA_HTML: &str = r#"<!doctype html>
<html><head><meta charset="utf-8">
<style>
  html, body { margin: 0; height: 100%; background: #323339; color: #f3f3f4;
    font: 15px -apple-system, "Segoe UI", sans-serif; }
  body { display: flex; flex-direction: column; align-items: center;
    justify-content: center; gap: 16px; }
</style>
<script>__POST_JS__</script>
<script>
  function onLoad() {
    const id = hcaptcha.render('captcha', {
      sitekey: __SITEKEY__,
      theme: 'dark',
      callback: (token) => emeraldPost(token),
    });
    const rqdata = __RQDATA__;
    if (rqdata) hcaptcha.setData(id, { rqdata });
  }
</script>
<script src="https://js.hcaptcha.com/1/api.js?onload=onLoad&render=explicit" async defer></script>
</head><body>
  <div>O Discord pediu uma verificação.</div>
  <div id="captcha"></div>
</body></html>"#;

/// Only the main frame of this host may post results back.
const TRUSTED_HOST: &str = "discord.com";

/// What a window shows: a remote page, or local HTML served as `base`.
enum Page<'a> {
    Url(&'a str),
    Html { html: &'a str, base: &'a str },
}

/// Opens the login window and blocks until the user authenticates or closes it.
/// Returns the captured account token, or `None` if the window was dismissed.
pub fn acquire_token() -> Option<String> {
    platform::show(
        "Entrar — Emerald",
        Page::Url("https://discord.com/login"),
        Some(&format!("{POST_JS}{CAPTURE_JS}")),
        (960.0, 760.0),
    )
}

/// Shows the hCaptcha widget for a challenge and returns the token the user
/// produced by solving it, or `None` if the window was closed.
pub fn solve_captcha(sitekey: &str, rqdata: Option<&str>) -> Option<String> {
    // JSON string literals are valid JS literals, which keeps server-provided
    // values from breaking out of the script.
    let html = CAPTCHA_HTML
        .replace("__POST_JS__", POST_JS)
        .replace("__SITEKEY__", &serde_json::to_string(sitekey).ok()?)
        .replace("__RQDATA__", &serde_json::to_string(&rqdata).ok()?);
    platform::show(
        "Verificação — Emerald",
        Page::Html {
            html: &html,
            base: "https://discord.com/",
        },
        None,
        (420.0, 640.0),
    )
}

/// Runs this binary with `flag` (child mode), feeding `input` on stdin, and
/// returns the trimmed stdout of a successful run.
pub fn run_child(flag: &str, input: &[u8]) -> Option<String> {
    use std::process::{Command, Stdio};

    let mut child = Command::new(std::env::current_exe().ok()?)
        .arg(flag)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .ok()?;
    child.stdin.take()?.write_all(input).ok()?;
    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        return None;
    }
    let result = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (!result.is_empty()).then_some(result)
}
