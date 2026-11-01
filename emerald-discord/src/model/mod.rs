//! Minimal Discord payload shapes. Only fields the UI renders are kept;
//! serde skips everything else, so large payloads (READY, GUILD_CREATE)
//! never materialize members or presences in memory.

mod channel;
mod collectibles;
mod command;
mod content_inventory;
mod devices;
mod emoji;
mod folders;
mod guild;
mod identity;
mod member_list;
mod message;
mod notifications;
mod profile;
pub(crate) mod proto;
mod saved;
mod session;
mod snowflake;
mod user;
pub(crate) mod user_settings;
mod voice;

pub use channel::*;
pub use collectibles::*;
pub use command::*;
pub use content_inventory::{Content, Feed, FeedEntry, InboxStale, content_type, trait_type};
pub use devices::*;
pub use emoji::*;
pub use folders::*;
pub use guild::*;
pub use identity::{GAMES_PER_REQUEST, Game, game_batches, GameAsset, GameMedia, NameStyle, PrimaryGuild, Widget, role_icon_url};
pub(crate) use identity::{lenient, lenient_list};
pub use member_list::*;
pub use message::*;
pub use notifications::*;
pub use profile::*;
pub use saved::*;
pub use session::*;
pub use snowflake::{DISCORD_EPOCH_MS, Snowflake};
pub use user::*;
pub use user_settings::{CustomStatus, Privacy, UserSettings, friend_source};
pub use voice::*;
