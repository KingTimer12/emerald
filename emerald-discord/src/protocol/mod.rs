//! The messages between the UI and the network thread: the UI sends
//! [`Command`]s, the network answers with [`NetEvent`]s.

mod account;
mod action;
mod command;
mod event;
mod forward;
mod moderation;
mod query;
mod voice;

pub use account::{OwnPresence, Picture, ProfileEdit, SettingsEdit};
pub use action::Action;
pub use command::{Command, IndexTarget, NewPoll, Page, ReactionSource, ReplyTo, Upload};
pub use moderation::{ChannelEdit, TagEdit};
pub use forward::{ForwardOf, ForwardStep, ForwardTo, forward_steps};
pub use event::{Link, NetEvent, PinPage, ReactionEvent, SearchPage, Snapshot, ThreadPage};
pub use query::{ProfileKind, MUTE_WINDOWS, SearchQuery, SearchScope, SettingsChange, ThreadQuery};
pub use crate::packets::saved::SaveSource;
pub use voice::{CallStatus, VoiceCall, VoiceControls, VoiceEvent};
