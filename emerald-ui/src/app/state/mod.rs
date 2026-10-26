//! What the client knows, kept apart from how it is drawn. Each store owns
//! one concern and is driven by plain method calls, so it can be tested
//! without a window.

pub mod account;
pub mod autocomplete;
pub mod commands;
pub mod drafts;
pub mod expiry;
pub mod folders;
pub mod hidden;
pub mod history;
pub mod member_search;
pub mod members;
pub mod msg;
pub mod notify;
pub mod people;
pub mod reads;
pub mod search;
pub mod spaces;
pub mod switcher;
pub mod typing;

pub use account::Account;
pub use autocomplete::Autocomplete;
pub use drafts::Drafts;
pub use expiry::TtlCache;
pub use folders::ServerList;
pub use history::{History, Log, MessageView};
pub use member_search::MemberSearch;
pub use members::{MemberList, MemberLists};
pub use notify::NotifySettings;
pub use msg::{FileCard, FileKind, Forward, Msg, Picture, Reacted, Reply, Sticker, sized_picture};
pub use people::People;
pub use reads::ReadStates;
pub use spaces::{ChannelRow, DMS, Row, Space, Spaces};
pub use typing::Typing;
