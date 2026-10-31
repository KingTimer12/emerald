//! Our own call as the UI follows it. Joining is two answers from the
//! gateway after op 4: our voice state (which carries the session) and the
//! voice server (endpoint and token). Once both are in, the voice engine
//! starts; a new server, or being moved, starts it again.

use rustc_hash::{FxHashMap, FxHashSet};

use emerald_discord::model::{CallUpdate, Snowflake, VoiceServer, VoiceState};
use emerald_discord::{CallStatus, VoiceCall};

pub struct Call {
    /// `None` for a call in DMs.
    pub guild: Option<Snowflake>,
    pub channel: Snowflake,
    pub status: CallStatus,
    /// Who is speaking now, us included.
    pub speaking: FxHashSet<Snowflake>,
    /// No microphone could be opened.
    pub listen_only: bool,
    session_id: Option<String>,
    server: Option<VoiceServer>,
    /// The engine runs with what we have; nothing to start.
    started: bool,
    /// We started this DM call: ring the others once Discord creates it.
    pub ring_on_create: bool,
    /// On a stage, in the audience: nothing we say is heard.
    pub suppressed: bool,
}

/// What our own voice state means for the call.
#[derive(Debug, PartialEq, Eq)]
pub enum Own {
    /// Still here, or now here.
    Joined,
    /// We left, or were disconnected by a moderator.
    Left,
}

impl Call {
    pub fn new(guild: Option<Snowflake>, channel: Snowflake) -> Self {
        Self {
            guild,
            channel,
            status: CallStatus::Connecting,
            speaking: FxHashSet::default(),
            listen_only: false,
            session_id: None,
            server: None,
            started: false,
            ring_on_create: false,
            suppressed: false,
        }
    }

    /// Our VOICE_STATE_UPDATE. Being moved to another channel keeps the call
    /// and restarts the engine there.
    pub fn own_state(&mut self, state: &VoiceState) -> Own {
        let Some(channel) = state.channel_id else { return Own::Left };
        if channel != self.channel {
            self.channel = channel;
            self.started = false;
            self.speaking.clear();
            self.status = CallStatus::Connecting;
        }
        self.suppressed = state.suppress;
        if self.session_id.as_deref() != Some(state.session_id.as_str()) {
            self.session_id = Some(state.session_id.clone());
            self.started = false;
        }
        Own::Joined
    }

    /// VOICE_SERVER_UPDATE for this call; false when it is for another.
    pub fn set_server(&mut self, server: VoiceServer) -> bool {
        let ours = match server.guild_id {
            Some(guild) => self.guild == Some(guild),
            None => self.guild.is_none() && server.channel_id.is_none_or(|c| c == self.channel),
        };
        if ours {
            self.server = Some(server);
            self.started = false;
            self.status = CallStatus::Connecting;
        }
        ours
    }

    /// The engine's credentials, once, when both halves are known.
    pub fn connect(&mut self, me: Snowflake) -> Option<VoiceCall> {
        if self.started {
            return None;
        }
        let session_id = self.session_id.clone()?;
        let server = self.server.as_ref()?;
        // Discord is still picking a server.
        let endpoint = server.endpoint.clone()?;
        self.started = true;
        Some(VoiceCall {
            guild: self.guild,
            channel: self.channel,
            user: me,
            session_id,
            token: server.token.clone(),
            endpoint,
        })
    }

    pub fn connected(&self) -> bool {
        matches!(self.status, CallStatus::Connected { .. })
    }
}

/// Calls going on in DMs and group DMs, whether or not we are in them.
#[derive(Default)]
pub struct DmCalls {
    calls: FxHashMap<Snowflake, DmCall>,
}

#[derive(Default)]
pub struct DmCall {
    /// Who is in it.
    pub users: Vec<Snowflake>,
    /// Whose phone rings for it.
    pub ringing: Vec<Snowflake>,
}

impl DmCalls {
    /// CALL_CREATE or CALL_UPDATE. True when it starts ringing `me`.
    pub fn update(&mut self, update: CallUpdate, me: Snowflake) -> bool {
        let call = self.calls.entry(update.channel_id).or_default();
        let rang = call.ringing.contains(&me);
        call.ringing = update.ringing;
        for state in update.voice_states {
            if state.channel_id == Some(update.channel_id) && !call.users.contains(&state.user_id) {
                call.users.push(state.user_id);
            }
        }
        !rang && call.ringing.contains(&me)
    }

    /// A voice state outside servers: someone joined or left a DM call.
    pub fn voice_state(&mut self, state: &VoiceState) {
        for (channel, call) in &mut self.calls {
            if state.channel_id != Some(*channel) {
                call.users.retain(|u| *u != state.user_id);
            }
        }
        if let Some(channel) = state.channel_id {
            let call = self.calls.entry(channel).or_default();
            if !call.users.contains(&state.user_id) {
                call.users.push(state.user_id);
            }
            call.ringing.retain(|u| *u != state.user_id);
        }
    }

    pub fn remove(&mut self, channel: Snowflake) {
        self.calls.remove(&channel);
    }

    /// The DM or group call `user` is in, if any.
    pub fn channel_of(&self, user: Snowflake) -> Option<Snowflake> {
        self.calls.iter().find(|(_, call)| call.users.contains(&user)).map(|(channel, _)| *channel)
    }

    pub fn get(&self, channel: Snowflake) -> Option<&DmCall> {
        self.calls.get(&channel).filter(|call| !call.users.is_empty() || !call.ringing.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(channel: Option<u64>, session: &str) -> VoiceState {
        serde_json::from_value(serde_json::json!({
            "guild_id": "1", "channel_id": channel.map(|c| c.to_string()), "user_id": "9", "session_id": session,
        }))
        .unwrap()
    }

    fn server(endpoint: Option<&str>) -> VoiceServer {
        serde_json::from_value(serde_json::json!({ "token": "t", "guild_id": "1", "endpoint": endpoint })).unwrap()
    }

    #[test]
    fn starts_once_both_answers_are_in() {
        let mut call = Call::new(Some(Snowflake(1)), Snowflake(2));
        assert!(call.connect(Snowflake(9)).is_none());
        assert_eq!(call.own_state(&state(Some(2), "s")), Own::Joined);
        assert!(call.connect(Snowflake(9)).is_none());
        assert!(call.set_server(server(Some("x.discord.media:443"))));
        let details = call.connect(Snowflake(9)).unwrap();
        assert_eq!((details.channel, details.session_id.as_str()), (Snowflake(2), "s"));
        assert!(call.connect(Snowflake(9)).is_none());
    }

    #[test]
    fn waits_while_discord_picks_a_server() {
        let mut call = Call::new(Some(Snowflake(1)), Snowflake(2));
        call.own_state(&state(Some(2), "s"));
        call.set_server(server(None));
        assert!(call.connect(Snowflake(9)).is_none());
        call.set_server(server(Some("y.discord.media")));
        assert!(call.connect(Snowflake(9)).is_some());
    }

    #[test]
    fn moving_restarts_and_leaving_ends() {
        let mut call = Call::new(Some(Snowflake(1)), Snowflake(2));
        call.own_state(&state(Some(2), "s"));
        call.set_server(server(Some("x.discord.media")));
        call.connect(Snowflake(9));
        assert_eq!(call.own_state(&state(Some(3), "s")), Own::Joined);
        assert_eq!(call.connect(Snowflake(9)).map(|c| c.channel), Some(Snowflake(3)));
        assert_eq!(call.own_state(&state(None, "s")), Own::Left);
    }

    #[test]
    fn dm_calls_follow_rings_and_voice_states() {
        let me = Snowflake(9);
        let mut calls = DmCalls::default();
        let update = |ringing: Vec<u64>| -> CallUpdate {
            serde_json::from_value(serde_json::json!({
                "channel_id": "5", "ringing": ringing.iter().map(u64::to_string).collect::<Vec<_>>(),
                "voice_states": [{ "user_id": "4", "channel_id": "5", "session_id": "x" }],
            }))
            .unwrap()
        };
        assert!(calls.update(update(vec![9]), me));
        // Still ringing is not a new ring.
        assert!(!calls.update(update(vec![9]), me));
        assert_eq!(calls.get(Snowflake(5)).unwrap().users, vec![Snowflake(4)]);
        let joined: VoiceState =
            serde_json::from_value(serde_json::json!({ "user_id": "9", "channel_id": "5", "session_id": "y" })).unwrap();
        calls.voice_state(&joined);
        let call = calls.get(Snowflake(5)).unwrap();
        assert_eq!((call.users.len(), call.ringing.len()), (2, 0));
        calls.remove(Snowflake(5));
        assert!(calls.get(Snowflake(5)).is_none());
    }

    #[test]
    fn other_servers_are_ignored() {
        let mut call = Call::new(Some(Snowflake(5)), Snowflake(2));
        assert!(!call.set_server(server(Some("x.discord.media"))));
    }
}
