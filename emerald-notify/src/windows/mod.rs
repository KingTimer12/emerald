//! Windows: toasts under an AppUserModelID registered for the current user,
//! the badge as an overlay icon on the taskbar button, and the icon in the
//! notification area.

mod badge;
mod toast;
pub mod tray;

use std::cell::RefCell;

use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
use windows_core::HSTRING;

use super::{App, Notice};

pub struct Platform {
    toasts: Option<toast::Toasts>,
    badge: RefCell<badge::Badge>,
}

impl Platform {
    pub fn new(app: &App, clicks: async_channel::Sender<String>) -> Self {
        // The taskbar groups the windows under the same id the toasts use,
        // so a click on a toast lands on this app's button.
        // SAFETY: a valid, NUL-terminated wide string.
        if let Err(e) = unsafe { SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(app.id)) } {
            tracing::warn!("notifications: app id: {e}");
        }
        let toasts = toast::Toasts::new(app, clicks)
            .map_err(|e| tracing::warn!("notifications: toasts unavailable: {e}"))
            .ok();
        Self {
            toasts,
            badge: RefCell::new(badge::Badge::default()),
        }
    }

    pub fn show(&self, notice: &Notice) -> Result<(), String> {
        let toasts = self.toasts.as_ref().ok_or("toasts unavailable")?;
        toasts.show(notice).map_err(|e| e.to_string())
    }

    pub fn set_badge(&self, count: u32) {
        self.badge.borrow_mut().set(count);
    }
}
