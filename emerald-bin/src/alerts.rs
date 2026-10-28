//! Notifications, the unread badge and the menu bar item through emerald-notify.

use std::rc::Rc;

use emerald_discord::model::Snowflake;
use emerald_notify::{App, Notice, Notifier, Tray, TrayAction};

/// The app id notifications and the taskbar button go under; also names
/// the pipe a second launch finds the first through.
pub const APP_ID: &str = "Emerald.Desktop";

/// emerald-notify behind the interface's `Alerts`. Cloned handles share
/// one notifier and one menu bar item.
#[derive(Clone)]
pub struct DesktopAlerts {
    notifier: Rc<Notifier>,
    tray: Rc<Tray>,
}

impl DesktopAlerts {
    pub fn new() -> Self {
        Self {
            notifier: Rc::new(Notifier::new(&App {
                id: APP_ID,
                name: "Emerald",
                icon: None,
            })),
            tray: Rc::new(Tray::new("Emerald", t!("tray.show"), t!("tray.quit"))),
        }
    }

    /// Channel ids of the notifications the user clicked, as text.
    pub fn clicks(&self) -> async_channel::Receiver<String> {
        self.notifier.clicks().clone()
    }

    /// Takes the tray icon away, as the app quits.
    pub fn remove_tray(&self) {
        self.tray.remove();
    }

    /// What the user picked in the menu bar item.
    pub fn tray_actions(&self) -> async_channel::Receiver<TrayAction> {
        self.tray.actions().clone()
    }
}

impl emerald_ui::Alerts for DesktopAlerts {
    fn notify(&self, title: &str, body: &str, channel: Snowflake) {
        let tag = channel.to_string();
        if let Err(e) = self.notifier.show(&Notice { title, body, tag: &tag }) {
            // The text stays out of the log: it is someone's message.
            tracing::warn!("notification not shown: {e}");
        }
    }

    fn badge(&self, count: u32) {
        self.notifier.set_badge(count);
        self.tray.set_count(count);
    }
}
