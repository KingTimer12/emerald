//! What the app asks of the system it runs on: opening on login and
//! updating itself. The executable implements [`Desktop`]; this side keeps
//! where updating stands and draws the "update ready" button.

use super::*;
use gpui::AnyElement;

use super::super::icons;

pub trait Desktop {
    /// Whether the app opens on login; `None` when this copy cannot (a
    /// build run from `target/`).
    fn opens_on_login(&self) -> Option<bool>;
    fn set_opens_on_login(&self, on: bool) -> Result<(), String>;
    /// Whether this copy updates itself (a bundle or an installed app).
    fn updates(&self) -> bool;
    /// Checks for an update now.
    fn check_for_updates(&self);
    /// Installs the downloaded update and arranges for it to open; the
    /// app quits after `Ok`.
    fn install_update(&self) -> Result<(), String>;
}

/// No system integration, for tests and builds run from `target/`.
impl Desktop for () {
    fn opens_on_login(&self) -> Option<bool> {
        None
    }
    fn set_opens_on_login(&self, _: bool) -> Result<(), String> {
        Err("unsupported".into())
    }
    fn updates(&self) -> bool {
        false
    }
    fn check_for_updates(&self) {}
    fn install_update(&self) -> Result<(), String> {
        Err("unsupported".into())
    }
}

/// Where updating stands.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Update {
    #[default]
    Idle,
    Checking,
    UpToDate,
    /// Downloading this version.
    Downloading(String),
    /// This version waits for a restart.
    Ready(String),
    /// The last check failed; it is tried again later.
    Failed,
}

impl Emerald {
    pub fn on_update(&mut self, update: Update, cx: &mut Context<Self>) {
        self.update = update;
        cx.notify();
    }

    /// Installs the waiting update and quits; the new version opens.
    pub(super) fn restart_to_update(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.desktop.install_update() {
            Ok(()) => window.dispatch_action(Box::new(crate::Quit), cx),
            Err(e) => {
                tracing::warn!("update not installed: {e}");
                self.update = Update::Failed;
                cx.notify();
            }
        }
    }

    pub(super) fn set_opens_on_login(&mut self, on: bool, cx: &mut Context<Self>) {
        if let Err(e) = self.desktop.set_opens_on_login(on) {
            tracing::warn!("open on login not changed: {e}");
        }
        self.opens_on_login = self.desktop.opens_on_login();
        cx.notify();
    }

    /// The green arrow atop the server list once an update is downloaded,
    /// as Discord shows it; a click restarts into it.
    pub(super) fn render_update_ready(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let Update::Ready(version) = &self.update else { return None };
        Some(
            div()
                .id("update-ready")
                .flex_none()
                .size(zoomed(40.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(zoomed(12.))
                .bg(rgb(theme::base_low()))
                .cursor_pointer()
                .hover(|s| s.bg(theme::hover()))
                .child(icons::icon(icons::DOWNLOAD, 20., theme::online()))
                .tooltip(widgets::tooltip(t!("update.ready", version = version)))
                .on_click(cx.listener(|this, _, window, cx| this.restart_to_update(window, cx)))
                .into_any_element(),
        )
    }
}
