//! Toasts. An app without a package or Start menu shortcut may still show
//! them once its AppUserModelID is described under the current user's
//! `Software\Classes\AppUserModelId` key, which is what this does first.

use std::cell::RefCell;
use std::collections::VecDeque;

use windows::Data::Xml::Dom::XmlDocument;
use windows::Foundation::TypedEventHandler;
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager, ToastNotifier};
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey, RegCreateKeyExW,
    RegSetValueExW,
};
use windows_core::{HSTRING, PCWSTR};

use super::super::{App, Notice};

/// Toasts kept alive so their click handlers still run from the Action
/// Center; older ones only open the app.
const KEEP: usize = 20;

pub struct Toasts {
    notifier: ToastNotifier,
    clicks: async_channel::Sender<String>,
    shown: RefCell<VecDeque<ToastNotification>>,
}

impl Toasts {
    pub fn new(app: &App, clicks: async_channel::Sender<String>) -> windows_core::Result<Self> {
        register(app);
        Ok(Self {
            notifier: ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(app.id))?,
            clicks,
            shown: RefCell::new(VecDeque::new()),
        })
    }

    pub fn show(&self, notice: &Notice) -> windows_core::Result<()> {
        let xml = XmlDocument::new()?;
        xml.LoadXml(&HSTRING::from(document(notice)))?;
        let toast = ToastNotification::CreateToastNotification(&xml)?;
        let clicks = self.clicks.clone();
        let tag = notice.tag.to_owned();
        // Runs on a thread of Windows' own; the channel carries it over.
        toast.Activated(&TypedEventHandler::new(move |_, _| {
            clicks.try_send(tag.clone()).ok();
            Ok(())
        }))?;
        self.notifier.Show(&toast)?;
        let mut shown = self.shown.borrow_mut();
        shown.push_back(toast);
        if shown.len() > KEEP {
            shown.pop_front();
        }
        Ok(())
    }
}

/// The toast's XML: a title line and the body, as plain text.
fn document(notice: &Notice) -> String {
    format!(
        "<toast><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual></toast>",
        escape(notice.title),
        escape(notice.body),
    )
}

fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            // XML 1.0 has no place for most control characters.
            c if c.is_control() && !matches!(c, '\n' | '\t') => {}
            c => escaped.push(c),
        }
    }
    escaped
}

/// Describes `app` to the notification platform: its name, and its icon
/// when there is one. Failing leaves Windows to show the id itself.
fn register(app: &App) {
    let path = HSTRING::from(format!("Software\\Classes\\AppUserModelId\\{}", app.id));
    let mut key = HKEY::default();
    // SAFETY: valid strings and an out pointer to a local key.
    let created = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            &path,
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut key,
            None,
        )
    };
    if created != ERROR_SUCCESS {
        tracing::warn!("notifications: could not register the app id ({created:?})");
        return;
    }
    set_string(key, "DisplayName", app.name);
    if let Some(icon) = &app.icon {
        set_string(key, "IconUri", &icon.to_string_lossy());
    }
    // SAFETY: the key opened above.
    unsafe { RegCloseKey(key).ok() }.ok();
}

fn set_string(key: HKEY, name: &str, value: &str) {
    let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: `wide` outlives the call; the byte view covers it exactly.
    let bytes = unsafe { std::slice::from_raw_parts(wide.as_ptr().cast::<u8>(), wide.len() * 2) };
    // SAFETY: an open key and a NUL-terminated name.
    let result = unsafe { RegSetValueExW(key, &HSTRING::from(name), None, REG_SZ, Some(bytes)) };
    if result != ERROR_SUCCESS {
        tracing::warn!("notifications: could not write {name} ({result:?})");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_cannot_break_out_of_the_document() {
        let notice = Notice {
            title: "Ana & <Bruno>",
            body: "</text><image src=\"x\"/>\u{1}",
            tag: "1",
        };
        let xml = document(&notice);
        assert!(xml.contains("Ana &amp; &lt;Bruno&gt;"));
        assert!(xml.contains("&lt;/text&gt;&lt;image src=&quot;x&quot;/&gt;</text>"));
        assert!(!xml.contains('\u{1}'));
    }
}
