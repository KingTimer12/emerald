//! Notifications and the unread badge through emerald-notify.

use std::rc::Rc;

use emerald_discord::model::Snowflake;
use emerald_notify::{App, Notice, Notifier};

/// The app id notifications and the taskbar button go under.
const APP_ID: &str = "Emerald.Desktop";

/// emerald-notify behind the interface's `Alerts`. Cloned handles share
/// one notifier.
#[derive(Clone)]
pub struct DesktopAlerts(Rc<Notifier>);

impl DesktopAlerts {
    pub fn new() -> Self {
        Self(Rc::new(Notifier::new(&App {
            id: APP_ID,
            name: "Emerald",
            icon: None,
        })))
    }

    /// Channel ids of the notifications the user clicked, as text.
    pub fn clicks(&self) -> async_channel::Receiver<String> {
        self.0.clicks().clone()
    }
}

impl emerald_ui::Alerts for DesktopAlerts {
    fn notify(&self, title: &str, body: &str, channel: Snowflake) {
        let tag = channel.to_string();
        if let Err(e) = self.0.show(&Notice { title, body, tag: &tag }) {
            // The text stays out of the log: it is someone's message.
            tracing::warn!("notification not shown: {e}");
        }
    }

    fn badge(&self, count: u32) {
        self.0.set_badge(count);
    }
}
