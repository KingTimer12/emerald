//! Bodies of the calls to ring people in a DM call, or stop ringing them.

use serde_json::{Value, json};

use crate::model::Snowflake;

/// `POST /channels/{id}/call/ring`. `None` rings everyone in the DM, as the
/// web client does when it starts a call.
pub fn ring(recipients: Option<&[Snowflake]>) -> Value {
    json!({ "recipients": recipients.map(|users| users.iter().map(Snowflake::to_string).collect::<Vec<_>>()) })
}

/// `POST /channels/{id}/call/stop-ringing`: declining a call stops our own
/// phone.
pub fn stop_ringing(recipients: &[Snowflake]) -> Value {
    json!({ "recipients": recipients.iter().map(Snowflake::to_string).collect::<Vec<_>>() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_everyone_or_someone() {
        assert_eq!(ring(None).to_string(), r#"{"recipients":null}"#);
        assert_eq!(stop_ringing(&[Snowflake(4)]).to_string(), r#"{"recipients":["4"]}"#);
    }
}
