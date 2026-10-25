//! Emerald's interface on gpui: the main window ([`Emerald`]) with its
//! areas and components, the text input, theme, icons and the local
//! [`settings`]. It reaches the network only through
//! `emerald_discord::Command` and `NetEvent`.

#[macro_use]
extern crate emerald_core;

mod app;
mod emoji;
mod icons;
mod lottie;
mod markdown;
pub mod settings;
pub mod text_input;
mod theme;
mod time;

pub use app::{Alerts, Emerald, bind_keys};
pub use icons::Icons;
pub use time::set_time_format;

gpui::actions!(emerald, [Logout]);
