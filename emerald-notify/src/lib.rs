//! Desktop notifications and a count on the app's icon, for apps that run
//! without an installer or bundle.
//!
//! ```no_run
//! use emerald_notify::{App, Notice, Notifier};
//!
//! let notifier = Notifier::new(&App { id: "Example.App", name: "Example", icon: None });
//! notifier.show(&Notice { title: "Ana", body: "hello", tag: "channel-1" });
//! notifier.set_badge(3);
//! while let Ok(tag) = notifier.clicks().try_recv() {
//!     println!("opened {tag}");
//! }
//! ```
//!
//! On Windows a notification is a toast under an AppUserModelID this crate
//! registers for the current user, and the badge is an overlay on the
//! taskbar button. On macOS the badge sits on the Dock icon; notifications
//! go through `osascript`, since Notification Center only accepts bundled
//! apps' own requests, so clicks are not reported there. Both work from
//! the thread that runs the app's windows.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

use std::path::PathBuf;

/// The app notifications are shown as.
#[derive(Clone, Debug)]
pub struct App<'a> {
    /// A stable identifier, such as `Company.Product`; Windows groups the
    /// app's notifications and taskbar button under it.
    pub id: &'a str,
    /// Shown as the notifications' source.
    pub name: &'a str,
    /// An image for the source, where the platform takes one.
    pub icon: Option<PathBuf>,
}

/// One notification.
#[derive(Clone, Copy, Debug)]
pub struct Notice<'a> {
    pub title: &'a str,
    pub body: &'a str,
    /// Handed back through [`Notifier::clicks`] when the user clicks it.
    pub tag: &'a str,
}

pub struct Notifier {
    #[cfg(windows)]
    platform: windows::Platform,
    #[cfg(target_os = "macos")]
    platform: macos::Platform,
    clicks: async_channel::Receiver<String>,
}

impl Notifier {
    pub fn new(app: &App) -> Self {
        let (sender, clicks) = async_channel::unbounded();
        #[cfg(not(any(windows, target_os = "macos")))]
        drop((app, sender));
        Self {
            #[cfg(windows)]
            platform: windows::Platform::new(app, sender),
            #[cfg(target_os = "macos")]
            platform: macos::Platform::new(app, sender),
            clicks,
        }
    }

    /// Shows `notice`. Failures are returned for logging; the app goes on
    /// either way.
    pub fn show(&self, notice: &Notice) -> Result<(), String> {
        #[cfg(any(windows, target_os = "macos"))]
        return self.platform.show(notice);
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            drop(notice);
            Err("notifications are not supported on this platform".into())
        }
    }

    /// Shows `count` on the app's icon; zero takes the badge away.
    pub fn set_badge(&self, count: u32) {
        #[cfg(any(windows, target_os = "macos"))]
        self.platform.set_badge(count);
        #[cfg(not(any(windows, target_os = "macos")))]
        drop(count);
    }

    /// Tags of the notifications the user clicked.
    pub fn clicks(&self) -> &async_channel::Receiver<String> {
        &self.clicks
    }
}

/// What the badge reads for `count`: the number up to nine, then "9+".
pub fn badge_text(count: u32) -> Option<String> {
    match count {
        0 => None,
        1..=9 => Some(count.to_string()),
        _ => Some("9+".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badges_cap_at_nine() {
        assert_eq!(badge_text(0), None);
        assert_eq!(badge_text(7).as_deref(), Some("7"));
        assert_eq!(badge_text(42).as_deref(), Some("9+"));
    }
}
