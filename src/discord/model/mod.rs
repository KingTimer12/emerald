//! Minimal Discord payload shapes. Only fields the UI renders are kept;
//! serde skips everything else, so large payloads (READY, GUILD_CREATE)
//! never materialize members, presences, roles or emojis in memory.

mod channel;
mod guild;
mod message;
mod profile;
mod session;
mod snowflake;
mod user;

pub use channel::*;
pub use guild::*;
pub use message::*;
pub use profile::*;
pub use session::*;
pub use snowflake::{DISCORD_EPOCH_MS, Snowflake};
pub use user::*;
