//! The WebView2 engine: its environment and controller, document-start
//! scripts, and local pages served under a remote origin.

use std::path::Path;
use std::sync::mpsc;

use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT, CreateCoreWebView2EnvironmentWithOptions, ICoreWebView2,
    ICoreWebView2Controller, ICoreWebView2Environment, ICoreWebView2Environment10,
};
use webview2_com::{
    AddScriptToExecuteOnDocumentCreatedCompletedHandler, CreateCoreWebView2ControllerCompletedHandler,
    CreateCoreWebView2EnvironmentCompletedHandler, WebResourceRequestedEventHandler, take_pwstr,
};
use windows::Win32::Foundation::{E_POINTER, ERROR_BUSY, HWND};
use windows::Win32::UI::Shell::SHCreateMemStream;
use windows::core::{HRESULT, HSTRING, Interface, PWSTR, w};

use super::Result;

/// Browser processes of a previous run that did not exit cleanly hold the
/// profile for a few seconds after their host dies; wait them out.
pub(super) fn create(hwnd: HWND, data_dir: Option<&Path>) -> Result<(ICoreWebView2Environment, ICoreWebView2Controller)> {
    const BUSY: HRESULT = ERROR_BUSY.to_hresult();
    let mut attempts = 0;
    loop {
        let created = create_environment(data_dir).and_then(|environment| {
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

/// The browser profile lives in `data_dir`; the session itself is InPrivate
/// (see `create_controller`), so nothing of the account stays.
fn create_environment(data_dir: Option<&Path>) -> Result<ICoreWebView2Environment> {
    let data_dir = data_dir.map(|dir| HSTRING::from(dir.as_os_str())).unwrap_or_default();
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
    // InPrivate session: nothing the page stores (cookies, a signed-in
    // session) outlives the window.
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
pub(super) fn add_script(webview: &ICoreWebView2, source: &str) -> Result<()> {
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
pub(super) fn serve(
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
