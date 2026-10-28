//! Desktop integration for an app shipped as a macOS bundle or through a
//! Windows installer:
//!
//! - [`autostart`]: open the app when the user logs in.
//! - [`instance`]: one instance at a time; a second launch hands its
//!   arguments (a link it was opened with) to the first and exits.
//! - [`update`]: check a release feed, download the build for this
//!   platform, check its ed25519 signature and install it on restart.
//!
//! Nothing here knows about the app it serves: ids, the feed's address and
//! the release key are the caller's.

pub mod autostart;
pub mod instance;
mod packaging;
pub mod update;

pub use packaging::Packaging;
