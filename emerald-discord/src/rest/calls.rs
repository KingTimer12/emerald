//! Calls in DMs: checking they can ring, ringing, and declining.

use anyhow::Result;

use super::Rest;
use crate::model::{CallInfo, Snowflake};
use crate::packets::call;
use emerald_http::Call;

impl Rest {
    /// Asked before starting a call, as the web client does: people who
    /// are not friends may not be ringable.
    pub async fn call_info(&self, channel: Snowflake) -> Result<CallInfo> {
        self.api
            .fetch(Call::get("/channels/{id}/call", format!("/channels/{channel}/call")).major(channel))
            .await
    }

    pub async fn ring(&self, channel: Snowflake, recipients: Option<&[Snowflake]>) -> Result<()> {
        self.api
            .run(
                Call::post("/channels/{id}/call/ring", format!("/channels/{channel}/call/ring"))
                    .major(channel)
                    .json(call::ring(recipients)),
            )
            .await
    }

    pub async fn stop_ringing(&self, channel: Snowflake, recipients: &[Snowflake]) -> Result<()> {
        self.api
            .run(
                Call::post("/channels/{id}/call/stop-ringing", format!("/channels/{channel}/call/stop-ringing"))
                    .major(channel)
                    .json(call::stop_ringing(recipients)),
            )
            .await
    }
}
