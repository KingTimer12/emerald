//! WebView2 backend: a plain Win32 window hosting a WebView2 controller,
//! pumped manually like the macOS one. WebView2 ships with Windows 11 (and
//! with Edge on Windows 10), so nothing extra is bundled.

use std::cell::{Cell, RefCell};
use std::sync::mpsc;

use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT,
    CreateCoreWebView2EnvironmentWithOptions, ICoreWebView2, ICoreWebView2Controller,
    ICoreWebView2Environment, ICoreWebView2Environment10,
};
use webview2_com::{
    AddScriptToExecuteOnDocumentCreatedCompletedHandler,
    CreateCoreWebView2ControllerCompletedHandler, CreateCoreWebView2EnvironmentCompletedHandler,
    WebMessageReceivedEventHandler, WebResourceRequestedEventHandler,
    WindowCloseRequestedEventHandler, take_pwstr,
};
use windows::Win32::Foundation::{
    E_POINTER, ERROR_BUSY, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM,
};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForSystem, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Shell::SHCreateMemStream;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect,
    GetSystemMetrics, IDC_ARROW, LoadCursorW, LoadIconW, MSG, MsgWaitForMultipleObjects, PM_REMOVE,
    PeekMessageW, QS_ALLINPUT, RegisterClassW, SM_CXSCREEN, SM_CYSCREEN, SW_SHOW,
    SetForegroundWindow, ShowWindow, TranslateMessage, WINDOW_EX_STYLE, WM_CLOSE, WM_SIZE,
    WNDCLASSW, WS_OVERLAPPEDWINDOW,
};
use windows::core::{HRESULT, HSTRING, Interface, PCWSTR, PWSTR, w};

use super::{Page, TRUSTED_HOST};

type Result<T> = std::result::Result<T, webview2_com::Error>;

/// Shared between the window procedure, the WebView2 event handlers and the
/// pump loop. The child process only ever has one window, on one thread.
#[derive(Default)]
struct WindowState {
    /// First non-empty message posted to `chrome.webview`.
    message: RefCell<Option<String>>,
    closed: Cell<bool>,
    /// Resized with the window.
    controller: RefCell<Option<ICoreWebView2Controller>>,
}

thread_local! {
    static STATE: WindowState = WindowState::default();
}

/// Opens a webview window and blocks until the page posts a message to
/// `chrome.webview` or the user closes the window.
pub(super) fn show(
    title: &str,
    page: Page,
    user_script: Option<&str>,
    size: (f64, f64),
) -> Option<String> {
    match run(title, page, user_script, size) {
        Ok(message) => message,
        Err(e) => {
            eprintln!(
                "could not open WebView2 (is the Edge WebView2 runtime installed?): {e:?}"
            );
            None
        }
    }
}

fn run(
    title: &str,
    page: Page,
    user_script: Option<&str>,
    size: (f64, f64),
) -> Result<Option<String>> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        // Crisp text on high-DPI screens; fails harmlessly if already set.
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    let hwnd = create_window(title, size)?;
    let (environment, controller) = create_webview(hwnd)?;
    fit(&controller, hwnd);
    STATE.with(|state| *state.controller.borrow_mut() = Some(controller.clone()));

    let webview = unsafe { controller.CoreWebView2()? };
    unsafe {
        let settings = webview.Settings()?;
        settings.SetAreDevToolsEnabled(false)?;
        settings.SetIsStatusBarEnabled(false)?;
    }

    let mut token = 0;
    unsafe {
        // `WebMessageReceived` on the webview only fires for the top-level
        // document; the source check keeps off-site pages out.
        webview.add_WebMessageReceived(
            &WebMessageReceivedEventHandler::create(Box::new(|_, args| {
                let Some(args) = args else { return Ok(()) };
                let mut source = PWSTR::null();
                args.Source(&mut source)?;
                if !is_trusted(&take_pwstr(source)) {
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
        add_script(&webview, source)?;
    }

    match page {
        Page::Url(url) => unsafe { webview.Navigate(&HSTRING::from(url))? },
        Page::Html { html, base } => {
            serve(&webview, &environment, base, html)?;
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

/// `https://discord.com/...` only: no other scheme, host, port or userinfo.
fn is_trusted(source: &str) -> bool {
    source
        .strip_prefix("https://")
        .and_then(|rest| rest.split(['/', '?', '#']).next())
        .is_some_and(|host| host == TRUSTED_HOST)
}

fn create_window(title: &str, size: (f64, f64)) -> Result<HWND> {
    unsafe {
        let instance = HINSTANCE(GetModuleHandleW(None)?.0);
        let class = w!("EmeraldWebView");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            // Resource 1 is the app icon (see build.rs).
            hIcon: LoadIconW(Some(instance), PCWSTR(1 as _)).unwrap_or_default(),
            lpszClassName: class,
            ..Default::default()
        };
        RegisterClassW(&wc);

        // `size` is in points, as on macOS.
        let scale = GetDpiForSystem() as f64 / 96.0;
        let (width, height) = ((size.0 * scale) as i32, (size.1 * scale) as i32);
        let x = (GetSystemMetrics(SM_CXSCREEN) - width).max(0) / 2;
        let y = (GetSystemMetrics(SM_CYSCREEN) - height).max(0) / 2;
        Ok(CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class,
            &HSTRING::from(title),
            WS_OVERLAPPEDWINDOW,
            x,
            y,
            width,
            height,
            None,
            None,
            Some(instance),
            None,
        )?)
    }
}

extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_SIZE => {
            STATE.with(|state| {
                if let Some(controller) = &*state.controller.borrow() {
                    fit(controller, hwnd);
                }
            });
            LRESULT(0)
        }
        // The pump loop sees the flag and tears the window down.
        WM_CLOSE => {
            STATE.with(|state| state.closed.set(true));
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// Makes the webview fill the window's client area.
fn fit(controller: &ICoreWebView2Controller, hwnd: HWND) {
    let mut rect = RECT::default();
    unsafe {
        if GetClientRect(hwnd, &mut rect).is_ok() {
            let _ = controller.SetBounds(rect);
        }
    }
}

/// Browser processes of a previous run that did not exit cleanly hold the
/// profile for a few seconds after their host dies; wait them out.
fn create_webview(hwnd: HWND) -> Result<(ICoreWebView2Environment, ICoreWebView2Controller)> {
    const BUSY: HRESULT = ERROR_BUSY.to_hresult();
    let mut attempts = 0;
    loop {
        let created = create_environment().and_then(|environment| {
            let controller = create_controller(&environment, hwnd)?;
            Ok((environment, controller))
        });
        match created {
            Err(webview2_com::Error::WindowsError(e)) if e.code() == BUSY && attempts < 20 => {
                attempts += 1;
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
            created => return created,
        }
    }
}

fn create_environment() -> Result<ICoreWebView2Environment> {
    // The browser profile lives with the cache; the session itself is
    // InPrivate (see `create_controller`), so nothing of the account stays.
    let data_dir = crate::paths::cache_dir()
        .map(|dir| HSTRING::from(dir.join("WebView2").as_os_str()))
        .unwrap_or_default();
    let (tx, rx) = mpsc::channel();
    CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| unsafe {
            CreateCoreWebView2EnvironmentWithOptions(None, &data_dir, None, &handler)
                .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |error_code, environment| {
            error_code?;
            tx.send(environment.ok_or_else(|| windows::core::Error::from(E_POINTER)))
                .expect("send over mpsc channel");
            Ok(())
        }),
    )?;
    Ok(rx.recv().map_err(|_| webview2_com::Error::SendError)??)
}

fn create_controller(
    environment: &ICoreWebView2Environment,
    hwnd: HWND,
) -> Result<ICoreWebView2Controller> {
    // InPrivate session: a persisted web session would replay the very token
    // that was just revoked, and logout must really log out. The token itself
    // is persisted by us, so nothing is lost.
    let environment: ICoreWebView2Environment10 = environment.cast()?;
    let options = unsafe { environment.CreateCoreWebView2ControllerOptions()? };
    unsafe { options.SetIsInPrivateModeEnabled(true)? };

    let (tx, rx) = mpsc::channel();
    CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| unsafe {
            environment
                .CreateCoreWebView2ControllerWithOptions(hwnd, &options, &handler)
                .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |error_code, controller| {
            error_code?;
            tx.send(controller.ok_or_else(|| windows::core::Error::from(E_POINTER)))
                .expect("send over mpsc channel");
            Ok(())
        }),
    )?;
    Ok(rx.recv().map_err(|_| webview2_com::Error::SendError)??)
}

/// Runs `source` at document start in every top-level document.
fn add_script(webview: &ICoreWebView2, source: &str) -> Result<()> {
    let webview = webview.clone();
    let source = HSTRING::from(source);
    AddScriptToExecuteOnDocumentCreatedCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| unsafe {
            webview
                .AddScriptToExecuteOnDocumentCreated(&source, &handler)
                .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(|error_code, _id| error_code),
    )
}

/// Answers document requests for exactly `url` with `html`, so a local page
/// gets that origin (WebView2 has no `loadHTMLString:baseURL:`).
fn serve(
    webview: &ICoreWebView2,
    environment: &ICoreWebView2Environment,
    url: &str,
    html: &str,
) -> Result<()> {
    unsafe {
        webview.AddWebResourceRequestedFilter(
            &HSTRING::from(url),
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT,
        )?
    };
    let environment = environment.clone();
    let url = url.to_owned();
    let html = html.as_bytes().to_vec();
    let mut token = 0;
    unsafe {
        webview.add_WebResourceRequested(
            &WebResourceRequestedEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else { return Ok(()) };
                let mut uri = PWSTR::null();
                args.Request()?.Uri(&mut uri)?;
                if take_pwstr(uri) != url {
                    return Ok(());
                }
                let stream = SHCreateMemStream(Some(&html));
                let response = environment.CreateWebResourceResponse(
                    stream.as_ref(),
                    200,
                    w!("OK"),
                    w!("Content-Type: text/html; charset=utf-8"),
                )?;
                args.SetResponse(&response)
            })),
            &mut token,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::is_trusted;

    #[test]
    fn only_discord_https_is_trusted() {
        assert!(is_trusted("https://discord.com/login"));
        assert!(is_trusted("https://discord.com/"));
        assert!(is_trusted("https://discord.com?x=1"));
        assert!(!is_trusted("http://discord.com/login"));
        assert!(!is_trusted("https://discord.com.evil.io/"));
        assert!(!is_trusted("https://discord.com@evil.io/"));
        assert!(!is_trusted("https://evil.io/discord.com"));
        assert!(!is_trusted("https://discord.com:8443/"));
    }
}
