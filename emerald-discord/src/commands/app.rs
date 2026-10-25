//! Apps: listing slash commands and running them.

use std::time::Duration;

use super::outcome::{or_expired, outcome};
use crate::model::Snowflake;
use crate::protocol::{Action, IndexTarget, NetEvent};
use crate::rest::{self, Rest};

/// Discord answers 202 while it builds the index; the web client asks
/// again 5 seconds later, three tries in all.
const INDEX_TRIES: usize = 3;
const INDEX_RETRY: Duration = Duration::from_secs(5);

pub async fn list(rest: &Rest, target: IndexTarget) -> NetEvent {
    for _ in 0..INDEX_TRIES {
        match rest.command_index(target).await {
            Ok(Some(index)) => return NetEvent::CommandIndex { target, index: Some(index) },
            Ok(None) => tokio::time::sleep(INDEX_RETRY).await,
            Err(e) => {
                return or_expired(e, |e| {
                    tracing::warn!("command index: {}", rest::describe(&e));
                    NetEvent::CommandIndex { target, index: None }
                });
            }
        }
    }
    NetEvent::CommandIndex { target, index: None }
}

/// The app answers through the gateway like any message.
pub async fn interact(
    rest: &Rest,
    application: Snowflake,
    guild: Option<Snowflake>,
    channel: Snowflake,
    data: serde_json::Value,
    nonce: String,
) -> NetEvent {
    outcome(Action::RunCommand, rest.run_command(application, guild, channel, data, &nonce).await)
}
