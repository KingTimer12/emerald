//! What the client knows, kept apart from how it is drawn. Each store owns
//! one concern and is driven by plain method calls, so it can be tested
//! without a window.

pub mod autocomplete;
pub mod commands;
pub mod drafts;
pub mod expiry;
pub mod history;
pub mod member_search;
pub mod members;
pub mod msg;
pub mod people;
pub mod reads;
pub mod spaces;
pub mod typing;

pub use autocomplete::Autocomplete;
pub use drafts::Drafts;
pub use expiry::TtlCache;
pub use history::{History, Log, MessageView};
pub use member_search::MemberSearch;
pub use members::{MemberList, MemberLists};
pub use msg::{Msg, Picture, Reacted, Reply, Sticker};
pub use people::People;
pub use reads::ReadStates;
pub use spaces::{ChannelRow, DMS, Row, Space, Spaces};
pub use typing::Typing;
