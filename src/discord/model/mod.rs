//! Minimal Discord payload shapes. Only fields the UI renders are kept;
//! serde skips everything else, so large payloads (READY, GUILD_CREATE)
//! never materialize members or presences in memory.

mod channel;
mod emoji;
mod guild;
mod member_list;
mod message;
mod profile;
mod session;
mod snowflake;
mod user;

pub use channel::*;
pub use emoji::*;
pub use guild::*;
pub use member_list::*;
pub use message::*;
pub use profile::*;
pub use session::*;
pub use snowflake::{DISCORD_EPOCH_MS, Snowflake};
pub use user::*;
