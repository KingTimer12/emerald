//! The app's icon in the notification area: a click shows the window, the
//! right button opens a menu to show it or quit, and the tooltip carries the
//! unread count. A hidden window of its own receives the icon's clicks.

use std::cell::RefCell;
use std::sync::OnceLock;

use async_channel::Sender;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow, GetCursorPos,
    GetSystemMetrics, HICON, IDI_APPLICATION, IMAGE_ICON, LR_DEFAULTCOLOR, LoadIconW, LoadImageW, MF_SEPARATOR,
    MF_STRING, PostMessageW, RegisterClassW, RegisterWindowMessageW, SM_CXSMICON, SetForegroundWindow,
    TPM_BOTTOMALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, WINDOW_EX_STYLE, WM_APP, WM_CONTEXTMENU,
    WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WNDCLASSW, WS_OVERLAPPED,
};
use windows_core::{HSTRING, PCWSTR, w};

use super::super::{TrayAction, badge_text};

/// The icon's clicks arrive as this message.
const CALLBACK: u32 = WM_APP + 1;
const SHOW: usize = 1;
const QUIT: usize = 2;

/// Where clicks go; set once, by the first icon made.
static SENDER: OnceLock<Sender<TrayAction>> = OnceLock::new();

thread_local! {
    /// The icon's data, to add it again when Explorer restarts, and the
    /// menu's titles. The window procedure runs on this same thread.
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

struct State {
    data: NOTIFYICONDATAW,
    show: HSTRING,
    quit: HSTRING,
    /// Explorer's "TaskbarCreated": the notification area is new and empty.
    taskbar_created: u32,
}

pub struct Icon {
    window: HWND,
    name: String,
}

impl Icon {
    /// Adds the icon. Call it on the thread that runs the app's windows.
    pub fn new(sender: Sender<TrayAction>, name: &str, show: &str, quit: &str) -> Option<Self> {
        let _ = SENDER.set(sender);
        // SAFETY: plain window class and window creation; the window lives
        // until `Drop` destroys it.
        let window = unsafe {
            let instance = GetModuleHandleW(None).ok()?;
            let class = WNDCLASSW {
                lpfnWndProc: Some(procedure),
                hInstance: instance.into(),
                lpszClassName: w!("EmeraldTrayIcon"),
                ..Default::default()
            };
            RegisterClassW(&class);
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("EmeraldTrayIcon"),
                w!(""),
                WS_OVERLAPPED,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(instance.into()),
                None,
            )
            .map_err(|e| tracing::warn!("tray: {e}"))
            .ok()?
        };
        let mut data = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: window,
            uID: 1,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: CALLBACK,
            hIcon: app_icon(),
            ..Default::default()
        };
        set_tip(&mut data, name);
        // SAFETY: `data` describes this process's own icon and window.
        if !unsafe { Shell_NotifyIconW(NIM_ADD, &data) }.as_bool() {
            tracing::warn!("tray: the icon was not added");
        }
        // SAFETY: a valid wide string.
        let taskbar_created = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };
        STATE.with_borrow_mut(|state| {
            *state = Some(State { data, show: show.into(), quit: quit.into(), taskbar_created });
        });
        Some(Self { window, name: name.to_owned() })
    }

    /// The tooltip reads the app's name and the unread count.
    pub fn set_count(&self, count: u32) {
        let tip = match badge_text(count) {
            Some(count) => format!("{} ({count})", self.name),
            None => self.name.clone(),
        };
        STATE.with_borrow_mut(|state| {
            let Some(state) = state else { return };
            set_tip(&mut state.data, &tip);
            // SAFETY: the icon added in `new`.
            let _ = unsafe { Shell_NotifyIconW(NIM_MODIFY, &state.data) };
        });
    }
}

impl Icon {
    /// Takes the icon out of the notification area, as the app quits; a
    /// process that ends with it there leaves it until hovered.
    pub fn remove(&self) {
        STATE.with_borrow_mut(|state| {
            if let Some(state) = state.take() {
                // SAFETY: the icon added in `new`.
                let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &state.data) };
            }
        });
    }
}

impl Drop for Icon {
    fn drop(&mut self) {
        self.remove();
        // SAFETY: the window made in `new`, on its own thread.
        unsafe { DestroyWindow(self.window).ok() };
    }
}

fn set_tip(data: &mut NOTIFYICONDATAW, tip: &str) {
    data.szTip = [0; 128];
    for (slot, unit) in data.szTip.iter_mut().take(127).zip(tip.encode_utf16()) {
        *slot = unit;
    }
}

/// The executable's own icon (resource 1), small; Windows' generic one
/// when it has none.
fn app_icon() -> HICON {
    // SAFETY: loads a shared icon resource of this module.
    unsafe {
        let size = GetSystemMetrics(SM_CXSMICON);
        let own = GetModuleHandleW(None)
            .and_then(|module| LoadImageW(Some(module.into()), PCWSTR(1 as _), IMAGE_ICON, size, size, LR_DEFAULTCOLOR));
        match own {
            Ok(handle) => HICON(handle.0),
            Err(_) => LoadIconW(None, IDI_APPLICATION).unwrap_or_default(),
        }
    }
}

fn send(action: TrayAction) {
    if let Some(sender) = SENDER.get() {
        let _ = sender.try_send(action);
    }
}

unsafe extern "system" fn procedure(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if message == CALLBACK {
        match lparam.0 as u32 & 0xffff {
            WM_LBUTTONUP => send(TrayAction::Show),
            WM_RBUTTONUP | WM_CONTEXTMENU => menu(window),
            _ => {}
        }
        return LRESULT(0);
    }
    let readd = STATE.with_borrow(|state| {
        state.as_ref().filter(|state| message == state.taskbar_created).map(|state| state.data)
    });
    if let Some(data) = readd {
        // SAFETY: the same icon, added again to Explorer's new taskbar.
        let _ = unsafe { Shell_NotifyIconW(NIM_ADD, &data) };
        return LRESULT(0);
    }
    // SAFETY: the default handling of everything else.
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

/// The right-click menu, at the cursor.
fn menu(window: HWND) {
    let Some((show, quit)) = STATE.with_borrow(|state| state.as_ref().map(|s| (s.show.clone(), s.quit.clone())))
    else {
        return;
    };
    // SAFETY: a menu made, shown and destroyed here; the window is ours.
    // Bringing it to the front first is what lets the menu close when the
    // user clicks elsewhere.
    unsafe {
        let Ok(popup) = CreatePopupMenu() else { return };
        let _ = AppendMenuW(popup, MF_STRING, SHOW, &show);
        let _ = AppendMenuW(popup, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(popup, MF_STRING, QUIT, &quit);
        let mut cursor = POINT::default();
        let _ = GetCursorPos(&mut cursor);
        let _ = SetForegroundWindow(window);
        let picked = TrackPopupMenu(
            popup,
            TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
            cursor.x,
            cursor.y,
            None,
            window,
            None,
        );
        let _ = PostMessageW(Some(window), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(popup);
        match picked.0 as usize {
            SHOW => send(TrayAction::Show),
            QUIT => send(TrayAction::Quit),
            _ => {}
        }
    }
}
