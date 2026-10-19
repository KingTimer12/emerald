//! Native `WKWebView` windows for the two flows that need Discord's own web
//! stack: the official login and hCaptcha challenges.
//!
//! The webview owns an `NSApplication`, and gpui insists on registering its
//! own `NSApplication` subclass as the very first `sharedApplication`, so each
//! flow runs in a child invocation of this binary (`--login`, `--captcha`)
//! that prints its result to stdout. Inside the child, events are pumped
//! manually instead of calling `-[NSApplication run]`.
//!
//! Token capture does not touch `localStorage` (Discord removed the token from
//! it). Instead an injected user script wraps `fetch` and `XMLHttpRequest` and
//! forwards any `Authorization` header to native code through a
//! `WKScriptMessageHandler`. That header is the raw account token the web
//! client sends on every `/api` request.

use std::cell::{Cell, RefCell};
use std::io::Write;
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSEventMask, NSWindow,
    NSWindowDelegate, NSWindowStyleMask,
};
use objc2_foundation::{
    MainThreadMarker, NSDate, NSDefaultRunLoopMode, NSNotification, NSPoint, NSRect, NSSize,
    NSString, NSURL, NSURLRequest,
};
use objc2_web_kit::{
    WKScriptMessage, WKScriptMessageHandler, WKUserContentController, WKUserScript,
    WKUserScriptInjectionTime, WKWebView, WKWebViewConfiguration, WKWebsiteDataStore,
};

/// Injected at document start. Wraps the two ways the web client sets request
/// headers and posts the `Authorization` value back exactly once.
const CAPTURE_JS: &str = r#"
(function () {
  let sent = false;
  function report(v) {
    if (sent || !v) return;
    v = String(v).trim();
    if (!v || v.toLowerCase().startsWith('bearer ')) return;
    sent = true;
    try { window.webkit.messageHandlers.emerald.postMessage(v); } catch (e) {}
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
/// the page is loaded with that base URL; the challenge itself is solved by
/// the user and the token goes back through the same message handler.
const CAPTCHA_HTML: &str = r#"<!doctype html>
<html><head><meta charset="utf-8">
<style>
  html, body { margin: 0; height: 100%; background: #323339; color: #f3f3f4;
    font: 15px -apple-system, sans-serif; }
  body { display: flex; flex-direction: column; align-items: center;
    justify-content: center; gap: 16px; }
</style>
<script>
  function onLoad() {
    const id = hcaptcha.render('captcha', {
      sitekey: __SITEKEY__,
      theme: 'dark',
      callback: (token) => window.webkit.messageHandlers.emerald.postMessage(token),
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

/// What a window shows: a remote page, or local HTML under a base URL.
enum Page<'a> {
    Url(&'a str),
    Html { html: &'a str, base: &'a str },
}

/// Shared between the Objective-C delegate and the Rust pump loop.
#[derive(Default)]
struct WindowState {
    /// First non-empty message posted to `messageHandlers.emerald`.
    message: RefCell<Option<String>>,
    closed: Cell<bool>,
}

struct Ivars {
    state: Rc<WindowState>,
}

define_class!(
    // SAFETY: subclass of NSObject, main-thread only, declares the protocols
    // whose methods it implements below.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "EmeraldWebViewDelegate"]
    #[ivars = Ivars]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl WKScriptMessageHandler for Delegate {
        #[unsafe(method(userContentController:didReceiveScriptMessage:))]
        fn did_receive(&self, _ucc: &WKUserContentController, message: &WKScriptMessage) {
            let body = unsafe { message.body() };
            if let Ok(s) = body.downcast::<NSString>() {
                let message = s.to_string();
                if !message.is_empty() {
                    *self.ivars().state.message.borrow_mut() = Some(message);
                }
            }
        }
    }

    unsafe impl NSWindowDelegate for Delegate {
        #[unsafe(method(windowWillClose:))]
        fn window_will_close(&self, _notification: &NSNotification) {
            self.ivars().state.closed.set(true);
        }
    }
);

impl Delegate {
    fn new(mtm: MainThreadMarker, state: Rc<WindowState>) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(Ivars { state });
        unsafe { msg_send![super(this), init] }
    }
}

/// Opens the login window and blocks until the user authenticates or closes it.
/// Returns the captured account token, or `None` if the window was dismissed.
pub fn acquire_token() -> Option<String> {
    show(
        "Entrar — Emerald",
        Page::Url("https://discord.com/login"),
        Some(CAPTURE_JS),
        (960.0, 760.0),
    )
}

/// Shows the hCaptcha widget for a challenge and returns the token the user
/// produced by solving it, or `None` if the window was closed.
pub fn solve_captcha(sitekey: &str, rqdata: Option<&str>) -> Option<String> {
    // JSON string literals are valid JS literals, which keeps server-provided
    // values from breaking out of the script.
    let html = CAPTCHA_HTML
        .replace("__SITEKEY__", &serde_json::to_string(sitekey).ok()?)
        .replace("__RQDATA__", &serde_json::to_string(&rqdata).ok()?);
    show(
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

/// Opens a webview window and blocks until the page posts a message to
/// `messageHandlers.emerald` or the user closes the window.
fn show(title: &str, page: Page, user_script: Option<&str>, size: (f64, f64)) -> Option<String> {
    let mtm = MainThreadMarker::new().expect("webview must run on the main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

    let state = Rc::new(WindowState::default());
    let delegate = Delegate::new(mtm, state.clone());

    let config = unsafe { WKWebViewConfiguration::new(mtm) };
    // Ephemeral session: a persisted web session would replay the very token
    // that was just revoked, and logout must really log out. The token itself
    // is persisted by us, so nothing is lost.
    unsafe { config.setWebsiteDataStore(&WKWebsiteDataStore::nonPersistentDataStore(mtm)) };
    let ucc = unsafe { config.userContentController() };
    if let Some(source) = user_script {
        let script = unsafe {
            WKUserScript::initWithSource_injectionTime_forMainFrameOnly(
                WKUserScript::alloc(mtm),
                &NSString::from_str(source),
                WKUserScriptInjectionTime::AtDocumentStart,
                true,
            )
        };
        unsafe { ucc.addUserScript(&script) };
    }
    unsafe {
        ucc.addScriptMessageHandler_name(
            ProtocolObject::from_ref(&*delegate),
            &NSString::from_str("emerald"),
        )
    };

    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(size.0, size.1));
    let webview = unsafe {
        WKWebView::initWithFrame_configuration(WKWebView::alloc(mtm), frame, &config)
    };
    match page {
        Page::Url(url) => {
            let url = NSURL::URLWithString(&NSString::from_str(url))?;
            unsafe { webview.loadRequest(&NSURLRequest::requestWithURL(&url)) };
        }
        Page::Html { html, base } => {
            let base = NSURL::URLWithString(&NSString::from_str(base))?;
            unsafe { webview.loadHTMLString_baseURL(&NSString::from_str(html), Some(&base)) };
        }
    }

    let style = NSWindowStyleMask::Titled
        | NSWindowStyleMask::Closable
        | NSWindowStyleMask::Resizable
        | NSWindowStyleMask::Miniaturizable;
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            frame,
            style,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    window.setTitle(&NSString::from_str(title));
    window.setContentView(Some(&webview));
    window.center();
    window.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    window.makeKeyAndOrderFront(None);
    app.activate();

    // Manual event pump: service AppKit until a message arrives or the window
    // closes. A short poll interval lets us re-check the flags even when no
    // event is pending.
    app.finishLaunching();
    loop {
        if state.message.borrow().is_some() || state.closed.get() {
            break;
        }
        let until = NSDate::dateWithTimeIntervalSinceNow(0.05);
        let event = unsafe {
            app.nextEventMatchingMask_untilDate_inMode_dequeue(
                NSEventMask::Any,
                Some(&until),
                NSDefaultRunLoopMode,
                true,
            )
        };
        if let Some(event) = event {
            app.sendEvent(&event);
        }
    }

    window.close();
    let message = state.message.borrow_mut().take();
    app.setActivationPolicy(NSApplicationActivationPolicy::Prohibited);
    message
}
