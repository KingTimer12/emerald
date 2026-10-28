//! Ringing people for a DM call, and declining one.

use super::outcome::outcome;
use crate::model::Snowflake;
use crate::protocol::{Action, NetEvent};
use crate::rest::Rest;

/// Rings everyone in the DM once our call exists, if Discord says they
/// can be rung.
pub async fn ring(rest: &Rest, channel: Snowflake) -> NetEvent {
    let rung = match rest.call_info(channel).await {
        Ok(info) if info.ringable => rest.ring(channel, None).await,
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    };
    outcome(Action::Call, rung)
}

pub async fn stop_ringing(rest: &Rest, channel: Snowflake, recipients: Vec<Snowflake>) -> NetEvent {
    outcome(Action::Call, rest.stop_ringing(channel, &recipients).await)
}
