//! Where the UI sends its requests. The network thread is one
//! implementation; tests record into a list instead.

use tokio::sync::mpsc::UnboundedSender;

use emerald_discord::Command;

pub trait Outbox {
    /// Queues `command`; false when nothing is listening anymore.
    fn post(&self, command: Command) -> bool;
}

impl Outbox for UnboundedSender<Command> {
    fn post(&self, command: Command) -> bool {
        self.send(command).is_ok()
    }
}

#[cfg(test)]
impl Outbox for std::cell::RefCell<Vec<Command>> {
    fn post(&self, command: Command) -> bool {
        self.borrow_mut().push(command);
        true
    }
}
