//! Minimal Discord payload shapes. Only fields the UI renders are kept;
//! serde skips everything else, so large payloads (READY, GUILD_CREATE)
//! never materialize members or presences in memory.

mod channel;
mod command;
mod emoji;
mod folders;
mod guild;
mod member_list;
mod message;
mod notifications;
mod profile;
mod session;
mod snowflake;
mod user;

pub use channel::*;
pub use command::*;
pub use emoji::*;
pub use folders::*;
pub use guild::*;
pub use member_list::*;
pub use message::*;
pub use notifications::*;
pub use profile::*;
pub use session::*;
pub use snowflake::{DISCORD_EPOCH_MS, Snowflake};
pub use user::*;
