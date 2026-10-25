//! WebView2 backend: a plain Win32 window ([`window`]) hosting a WebView2
//! controller ([`engine`]), pumped manually like the macOS one. WebView2
//! ships with Windows 11 (and with Edge on Windows 10), so nothing extra is
//! bundled.

mod engine;
mod window;

use std::cell::{Cell, RefCell};

use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC, ICoreWebView2Controller,
};
use webview2_com::{WebMessageReceivedEventHandler, WindowCloseRequestedEventHandler, take_pwstr};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, DispatchMessageW, MSG, MsgWaitForMultipleObjects, PM_REMOVE, PeekMessageW, QS_ALLINPUT,
    SW_SHOW, SetForegroundWindow, ShowWindow, TranslateMessage,
};
use windows::core::{HSTRING, PWSTR};

use super::{Backend, Error, Page, WebView, is_trusted};

type Result<T> = std::result::Result<T, webview2_com::Error>;

/// Shared between the window procedure, the WebView2 event handlers and the
/// pump loop. The child process only ever has one window, on one thread.
#[derive(Default)]
pub(super) struct WindowState {
    /// First non-empty message posted to `chrome.webview`.
    pub(super) message: RefCell<Option<String>>,
    pub(super) closed: Cell<bool>,
    /// Resized with the window.
    pub(super) controller: RefCell<Option<ICoreWebView2Controller>>,
}

thread_local! {
    pub(super) static STATE: WindowState = WindowState::default();
}

/// WebView2 in a Win32 window, posting through `chrome.webview`.
pub struct WebView2;

impl Backend for WebView2 {
    fn show(view: &WebView<'_>) -> std::result::Result<Option<String>, Error> {
        run(view).map_err(|e| Error(format!("could not open WebView2 (is the Edge WebView2 runtime installed?): {e:?}")))
    }
}

/// Opens the window and blocks until the page posts a message or the user
/// closes the window.
fn run(view: &WebView<'_>) -> Result<Option<String>> {
    let WebView {
        title,
        page,
        script: user_script,
        size,
        trusted_host,
        ..
    } = *view;
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        // Crisp text on high-DPI screens; fails harmlessly if already set.
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    let hwnd = window::create(title, size)?;
    let (environment, controller) = engine::create(hwnd, view.data_dir.as_deref())?;
    window::fit(&controller, hwnd);
    STATE.with(|state| *state.controller.borrow_mut() = Some(controller.clone()));

    let webview = unsafe { controller.CoreWebView2()? };
    unsafe {
        let settings = webview.Settings()?;
        settings.SetAreDevToolsEnabled(false)?;
        settings.SetIsStatusBarEnabled(false)?;
    }

    // The handler outlives this call's borrows.
    let trusted_host = trusted_host.to_owned();
    let mut token = 0;
    unsafe {
        // `WebMessageReceived` on the webview only fires for the top-level
        // document; the source check keeps off-site pages out.
        webview.add_WebMessageReceived(
            &WebMessageReceivedEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else { return Ok(()) };
                let mut source = PWSTR::null();
                args.Source(&mut source)?;
                if !is_trusted(&take_pwstr(source), &trusted_host) {
                    return Ok(());
                }
                let mut body = PWSTR::null();
                if args.TryGetWebMessageAsString(&mut body).is_ok() {
                    let message = take_pwstr(body);
                    if !message.is_empty() {
                        STATE.with(|state| *state.message.borrow_mut() = Some(message));
                    }
                }
                Ok(())
            })),
            &mut token,
        )?;
        // `window.close()` from the page.
        webview.add_WindowCloseRequested(
            &WindowCloseRequestedEventHandler::create(Box::new(|_, _| {
                STATE.with(|state| state.closed.set(true));
                Ok(())
            })),
            &mut token,
        )?;
    }

    if let Some(source) = user_script {
        engine::add_script(&webview, source)?;
    }

    match page {
        Page::Url(url) => unsafe { webview.Navigate(&HSTRING::from(url))? },
        Page::Html { html, base } => {
            engine::serve(&webview, &environment, base, html)?;
            unsafe { webview.Navigate(&HSTRING::from(base))? };
        }
    }

    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        controller.SetIsVisible(true)?;
        controller.MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC)?;
    }

    // Manual event pump: service the message queue until a message arrives or
    // the window closes. The timeout re-checks the flags even when idle.
    let mut msg = MSG::default();
    while !STATE.with(|state| state.message.borrow().is_some() || state.closed.get()) {
        unsafe {
            MsgWaitForMultipleObjects(None, false, 50, QS_ALLINPUT);
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    STATE.with(|state| state.controller.borrow_mut().take());
    unsafe {
        let _ = controller.Close();
        let _ = DestroyWindow(hwnd);
    }
    Ok(STATE.with(|state| state.message.borrow_mut().take()))
}
