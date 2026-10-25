//! The gateway link as the status bar words it, and result lines under forms.

use super::*;

/// Result line under a form, green on success and red on failure.
pub(super) struct Feedback {
    pub(super) ok: bool,
    pub(super) text: SharedString,
}

impl Feedback {
    pub(super) fn from_result(result: Result<(), String>, success: impl Into<SharedString>) -> Self {
        match result {
            Ok(()) => Self {
                ok: true,
                text: success.into(),
            },
            Err(error) => Self {
                ok: false,
                text: error.into(),
            },
        }
    }
}

/// The gateway connection as the user panel shows it.
pub(super) struct Connection {
    /// The last state the gateway reported.
    pub(super) link: Link,
    pub(super) status: SharedString,
    /// `status` is an error or "logging out" rather than the link's state.
    pub(super) custom: bool,
    /// Session is live; drives the user panel's status dot.
    pub(super) connected: bool,
    pub(super) fatal: Option<SharedString>,
}

impl Connection {
    pub(super) fn set_link(&mut self, link: Link) {
        self.link = link;
        self.custom = false;
        self.connected = link == Link::Connected;
        self.status = match link {
            Link::Connecting => t!("connection.connecting").into(),
            Link::Resuming => t!("connection.resuming").into(),
            Link::Connected => t!("connection.connected").into(),
            Link::Reconnecting(secs) => t!("connection.reconnecting", secs = secs).into(),
        };
    }

    /// Shows a message other than the link's state.
    pub(super) fn set_message(&mut self, message: impl Into<SharedString>) {
        self.status = message.into();
        self.custom = true;
    }

    /// Words the link's state again, in a new language.
    pub(super) fn relabel(&mut self) {
        if !self.custom {
            self.set_link(self.link);
        }
    }
}
