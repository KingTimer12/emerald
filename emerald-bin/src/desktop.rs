//! Opening on login and updates through emerald-desktop, behind the
//! interface's `Desktop`.

use std::rc::Rc;

use emerald_core::paths;
use emerald_desktop::update::{Config, Status, Updater};
use emerald_desktop::{Packaging, autostart};
use emerald_ui::Update;

/// The LaunchAgent's label on macOS (the bundle's identifier) and the
/// `Run` value's name on Windows.
#[cfg(target_os = "macos")]
const LOGIN_ID: &str = "io.github.kingtimer12.emerald";
#[cfg(not(target_os = "macos"))]
const LOGIN_ID: &str = "Emerald";

/// Published with every release (see `.github/workflows/release.yml`).
const FEED: &str = "https://github.com/KingTimer12/emerald/releases/latest/download/latest.json";

/// The release key's public half. Its secret stays with whoever publishes
/// releases; `cargo run -p emerald-desktop --example release_key` explains.
const RELEASE_KEY: &str = "wdnrn9V93CcaHbrqa+fZTeAjDRcWnIpPU3kJ63+S8Gw=";

#[derive(Clone)]
pub struct DesktopIntegration {
    packaging: Option<Packaging>,
    updater: Option<Rc<Updater>>,
}

impl DesktopIntegration {
    /// Starts checking for updates when this copy is packaged.
    pub fn new() -> Self {
        let packaging = Packaging::current();
        let updater = packaging.clone().and_then(|packaging| {
            let config = Config {
                feed: FEED.into(),
                key: RELEASE_KEY.into(),
                version: env!("CARGO_PKG_VERSION").into(),
                user_agent: format!("Emerald/{}", env!("CARGO_PKG_VERSION")),
                dir: paths::cache_dir()?.join("updates"),
                packaging,
            };
            Updater::start(config).map_err(|e| tracing::warn!("updates off: {e:#}")).ok().map(Rc::new)
        });
        Self { packaging, updater }
    }

    /// Where updating stands, as it changes; `None` when updates are off.
    pub fn updates(&self) -> Option<async_channel::Receiver<Status>> {
        self.updater.as_ref().map(|updater| updater.status().clone())
    }
}

/// The interface's side of an update status.
pub fn interface(status: Status) -> Update {
    match status {
        Status::Checking => Update::Checking,
        Status::UpToDate => Update::UpToDate,
        Status::Downloading(version) => Update::Downloading(version),
        Status::Ready(version) => Update::Ready(version),
        Status::Failed(_) => Update::Failed,
    }
}

impl emerald_ui::Desktop for DesktopIntegration {
    fn opens_on_login(&self) -> Option<bool> {
        self.packaging.is_some().then(|| autostart::enabled(LOGIN_ID))
    }

    fn set_opens_on_login(&self, on: bool) -> Result<(), String> {
        tracing::info!(on, "open on login");
        autostart::set(LOGIN_ID, on).map_err(|e| e.to_string())
    }

    fn updates(&self) -> bool {
        self.updater.is_some()
    }

    fn check_for_updates(&self) {
        if let Some(updater) = &self.updater {
            updater.check();
        }
    }

    fn install_update(&self) -> Result<(), String> {
        let updater = self.updater.as_ref().ok_or("updates are off")?;
        updater.install().map_err(|e| format!("{e:#}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_release_key_and_feed_are_usable() {
        assert!(emerald_desktop::update::public_key(RELEASE_KEY).is_some());
        assert!(FEED.starts_with("https://"));
    }
}
