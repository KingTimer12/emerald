//! "… is typing": who types where, and our own throttled signal.

use std::time::{Duration, Instant};

use rustc_hash::FxHashMap;

use super::expiry::{Deadlines, Throttle};
use emerald_discord::model::Snowflake;

/// The web client re-sends the typing signal about this often while typing.
const INTERVAL: Duration = Duration::from_secs(8);
/// How long someone shows as typing after their last signal.
pub const TIMEOUT: Duration = Duration::from_secs(10);

pub struct Typing {
    channels: FxHashMap<Snowflake, Deadlines<Snowflake>>,
    sent: Throttle<Snowflake>,
}

impl Default for Typing {
    fn default() -> Self {
        Self {
            channels: FxHashMap::default(),
            sent: Throttle::new(INTERVAL),
        }
    }
}

impl Typing {
    pub fn start(&mut self, channel: Snowflake, user: Snowflake, now: Instant) {
        self.channels.entry(channel).or_default().set(user, now + TIMEOUT);
    }

    /// A message ends its author's indicator.
    pub fn stop(&mut self, channel: Snowflake, user: Snowflake) {
        if let Some(users) = self.channels.get_mut(&channel) {
            users.remove(&user);
        }
    }

    pub fn prune(&mut self, now: Instant) {
        self.channels.retain(|_, users| {
            users.prune(now);
            !users.is_empty()
        });
    }

    pub fn who(&self, channel: Snowflake) -> impl Iterator<Item = Snowflake> + '_ {
        self.channels.get(&channel).into_iter().flat_map(|users| users.iter().copied())
    }

    /// Whether to tell Discord we are typing in `channel` now.
    pub fn should_signal(&mut self, channel: Snowflake, now: Instant) -> bool {
        self.sent.allow(channel, now)
    }

    /// A send ends our typing, so the next keystroke signals again.
    pub fn sent(&mut self) {
        self.sent.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indicators_expire_and_stop() {
        let now = Instant::now();
        let mut typing = Typing::default();
        typing.start(Snowflake(1), Snowflake(5), now);
        typing.start(Snowflake(1), Snowflake(6), now + Duration::from_secs(5));
        typing.stop(Snowflake(1), Snowflake(6));
        assert_eq!(typing.who(Snowflake(1)).collect::<Vec<_>>(), [Snowflake(5)]);
        typing.prune(now + TIMEOUT);
        assert_eq!(typing.who(Snowflake(1)).count(), 0);
    }
}
