//! The messages between the UI and the network thread: the UI sends
//! [`Command`]s, the network answers with [`NetEvent`]s.

mod action;
mod command;
mod event;

pub use action::Action;
pub use command::{Command, IndexTarget, NewPoll, Page, ReactionSource, ReplyTo, Upload};
pub use event::{Link, NetEvent, ReactionEvent, Snapshot, ThreadPage};
