//! Who is in which voice channel, and where to connect to talk there.

use serde::Deserialize;

use super::{Member, Snowflake};

/// One user's place in voice: READY and GUILD_CREATE list them per guild,
/// VOICE_STATE_UPDATE changes one. `channel_id` is `None` once they left.
#[derive(Clone, Debug, Deserialize)]
pub struct VoiceState {
    /// Missing inside a guild's own list; filled in from the guild.
    #[serde(default)]
    pub guild_id: Option<Snowflake>,
    #[serde(default)]
    pub channel_id: Option<Snowflake>,
    pub user_id: Snowflake,
    /// The gateway session that is in the call; the voice server wants ours.
    #[serde(default)]
    pub session_id: String,
    /// Updates carry the member, so the sidebar can name them.
    #[serde(default)]
    pub member: Option<Member>,
    /// Muted or deafened by a moderator.
    #[serde(default)]
    pub mute: bool,
    #[serde(default)]
    pub deaf: bool,
    #[serde(default)]
    pub self_mute: bool,
    #[serde(default)]
    pub self_deaf: bool,
    #[serde(default)]
    pub self_video: bool,
    /// Sharing their screen (Go Live).
    #[serde(default)]
    pub self_stream: bool,
    /// On a stage, in the audience.
    #[serde(default)]
    pub suppress: bool,
}

impl VoiceState {
    /// Can be heard: not muted by themselves or a moderator, nor suppressed.
    pub fn speaks(&self) -> bool {
        !(self.mute || self.self_mute || self.suppress)
    }
}

/// VOICE_SERVER_UPDATE: where our voice connection goes, and the token for
/// it. `endpoint` is `None` while Discord picks a new server.
#[derive(Clone, Debug, Deserialize)]
pub struct VoiceServer {
    pub token: String,
    #[serde(default)]
    pub guild_id: Option<Snowflake>,
    /// Set instead of `guild_id` for calls in DMs.
    #[serde(default)]
    pub channel_id: Option<Snowflake>,
    #[serde(default)]
    pub endpoint: Option<String>,
}

/// CALL_CREATE and CALL_UPDATE: a call in a DM or group DM, and whose
/// phones ring for it.
#[derive(Clone, Debug, Deserialize)]
pub struct CallUpdate {
    pub channel_id: Snowflake,
    #[serde(default)]
    pub ringing: Vec<Snowflake>,
    /// CALL_CREATE lists who is in it already.
    #[serde(default)]
    pub voice_states: Vec<VoiceState>,
}

/// CALL_DELETE: the call in `channel_id` is over.
#[derive(Clone, Debug, Deserialize)]
pub struct CallDelete {
    pub channel_id: Snowflake,
}

/// `GET /channels/{id}/call`: whether the others can be rung.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct CallInfo {
    #[serde(default)]
    pub ringable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_state_from_an_update() {
        let state: VoiceState = serde_json::from_str(
            r#"{"guild_id":"1","channel_id":"2","user_id":"3","session_id":"abc","self_mute":true,
                "member":{"user":{"id":"3","username":"ana"},"roles":[]},"request_to_speak_timestamp":null}"#,
        )
        .unwrap();
        assert_eq!(state.channel_id, Some(Snowflake(2)));
        assert!(!state.speaks());
        assert_eq!(state.member.and_then(|m| m.id()), Some(Snowflake(3)));
    }

    #[test]
    fn call_from_a_create() {
        let call: CallUpdate = serde_json::from_str(
            r#"{"channel_id":"5","message_id":"6","region":"brazil","ringing":["7"],
                "voice_states":[{"user_id":"8","channel_id":"5","session_id":"x"}]}"#,
        )
        .unwrap();
        assert_eq!((call.ringing, call.voice_states.len()), (vec![Snowflake(7)], 1));
    }
}
