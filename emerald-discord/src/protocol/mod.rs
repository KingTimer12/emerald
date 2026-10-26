//! The messages between the UI and the network thread: the UI sends
//! [`Command`]s, the network answers with [`NetEvent`]s.

mod account;
mod action;
mod command;
mod event;
mod query;

pub use account::{OwnPresence, Picture, ProfileEdit, SettingsEdit};
pub use action::Action;
pub use command::{Command, IndexTarget, NewPoll, Page, ReactionSource, ReplyTo, Upload};
pub use event::{Link, NetEvent, PinPage, ReactionEvent, SearchPage, Snapshot, ThreadPage};
pub use query::{MUTE_WINDOWS, SearchQuery, SearchScope, SettingsChange, ThreadQuery};
