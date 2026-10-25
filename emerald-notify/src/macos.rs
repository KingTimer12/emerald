//! macOS: the badge on the Dock icon, and notifications through
//! `osascript`. Notification Center only takes requests from bundled apps
//! (`UNUserNotificationCenter` aborts without a bundle), so clicks are not
//! reported here.

use std::process::{Command, Stdio};

use objc2::MainThreadMarker;
use objc2_app_kit::NSApplication;
use objc2_foundation::NSString;

use super::{App, Notice, badge_text};

pub struct Platform;

impl Platform {
    pub fn new(_app: &App, _clicks: async_channel::Sender<String>) -> Self {
        Self
    }

    pub fn show(&self, notice: &Notice) -> Result<(), String> {
        // The text goes in as arguments, never into the script itself.
        Command::new("/usr/bin/osascript")
            .args([
                "-e",
                "on run argv",
                "-e",
                "display notification (item 2 of argv) with title (item 1 of argv)",
                "-e",
                "end run",
                notice.title,
                notice.body,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(drop)
            .map_err(|e| e.to_string())
    }

    pub fn set_badge(&self, count: u32) {
        let Some(main_thread) = MainThreadMarker::new() else {
            tracing::warn!("badge: not on the main thread");
            return;
        };
        let label = badge_text(count).map(|text| NSString::from_str(&text));
        let app = NSApplication::sharedApplication(main_thread);
        app.dockTile().setBadgeLabel(label.as_deref());
    }
}
