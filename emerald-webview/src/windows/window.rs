//! The Win32 window that hosts the webview.

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::UI::HiDpi::GetDpiForSystem;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetClientRect, GetSystemMetrics, IDC_ARROW, LoadCursorW, LoadIconW,
    RegisterClassW, SM_CXSCREEN, SM_CYSCREEN, WINDOW_EX_STYLE, WM_CLOSE, WM_SIZE, WNDCLASSW, WS_OVERLAPPEDWINDOW,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::core::{HSTRING, PCWSTR, w};
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller;

use super::{Result, STATE};

/// A top-level window of `size` points, centered on the screen.
pub(super) fn create(title: &str, size: (f64, f64)) -> Result<HWND> {
    unsafe {
        let instance = HINSTANCE(GetModuleHandleW(None)?.0);
        let class = w!("EmeraldWebView");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            // Resource 1 is the executable's icon, when it embeds one.
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
pub(super) fn fit(controller: &ICoreWebView2Controller, hwnd: HWND) {
    let mut rect = RECT::default();
    unsafe {
        if GetClientRect(hwnd, &mut rect).is_ok() {
            let _ = controller.SetBounds(rect);
        }
    }
}
