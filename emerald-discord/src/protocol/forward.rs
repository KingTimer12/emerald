//! Forwarding a message to other channels, as the web client's "Forward
//! To" modal does.

use crate::model::Snowflake;

/// The message being forwarded. `guild` is the source's, `None` in a DM.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ForwardOf {
    pub guild: Option<Snowflake>,
    pub channel: Snowflake,
    pub message: Snowflake,
}

/// Where a forward goes, and whether the optional text follows it there.
/// The web client leaves the text out of slowmode channels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ForwardTo {
    pub channel: Snowflake,
    pub with_text: bool,
}

/// One request of a forward.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForwardStep {
    /// The forward itself: an empty message referencing the original.
    Forward(Snowflake),
    /// The optional text, sent only once that channel's forward went out.
    Text(Snowflake),
}

/// The requests of a forward in order: each destination's forward, then
/// its text when there is one.
pub fn forward_steps(to: &[ForwardTo], text: Option<&str>) -> Vec<ForwardStep> {
    let text = text.is_some_and(|t| !t.trim().is_empty());
    let mut steps = Vec::with_capacity(to.len() * 2);
    for dest in to {
        steps.push(ForwardStep::Forward(dest.channel));
        if text && dest.with_text {
            steps.push(ForwardStep::Text(dest.channel));
        }
    }
    steps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_text_follows_its_forward() {
        let to = [
            ForwardTo { channel: Snowflake(1), with_text: true },
            ForwardTo { channel: Snowflake(2), with_text: false },
        ];
        assert_eq!(
            forward_steps(&to, Some("hi")),
            [ForwardStep::Forward(Snowflake(1)), ForwardStep::Text(Snowflake(1)), ForwardStep::Forward(Snowflake(2))]
        );
        assert_eq!(forward_steps(&to, Some("  ")).len(), 2);
        assert_eq!(forward_steps(&to, None).len(), 2);
    }
}
