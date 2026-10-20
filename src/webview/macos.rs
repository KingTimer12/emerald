//! `WKWebView` backend. The webview owns an `NSApplication`, and gpui registers
//! its own `NSApplication` subclass as the very first `sharedApplication`,
//! hence the child process (see the parent module).

use std::cell::{Cell, RefCell};
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

use super::{Page, TRUSTED_HOST};

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
            // Every frame can reach `messageHandlers`; only trust the
            // discord.com main frame, never iframes or off-site pages.
            let frame = unsafe { message.frameInfo() };
            let origin = unsafe { frame.securityOrigin() };
            let trusted = unsafe {
                frame.isMainFrame()
                    && origin.protocol().to_string() == "https"
                    && origin.host().to_string() == TRUSTED_HOST
            };
            if !trusted {
                return;
            }
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

/// Opens a webview window and blocks until the page posts a message to
/// `messageHandlers.emerald` or the user closes the window.
pub(super) fn show(
    title: &str,
    page: Page,
    user_script: Option<&str>,
    size: (f64, f64),
) -> Option<String> {
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
