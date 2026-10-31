//! Voice channels, stages and DM calls: joining and leaving, mute and
//! deafen, and what the gateway and the voice engine say about calls. The
//! call panel, the people in voice and the DM call bar are drawn in
//! [`panel`], [`rows`] and [`dm`].

pub(super) mod audio_menu;
mod dm;
mod panel;
mod rows;

use gpui::{Context, SharedString};

use super::Emerald;
use super::state::call::{Call, Own};
use emerald_discord::model::{Snowflake, VoiceServer, VoiceState};
use emerald_discord::permissions;
use emerald_discord::{CallStatus, Command, VoiceControls, VoiceEvent};

impl Emerald {
    /// Joins voice `channel`, leaving any other call. Refused, with a
    /// notice, without the right to connect or when the channel is full.
    pub(super) fn join_voice(&mut self, guild: Snowflake, channel: Snowflake, cx: &mut Context<Self>) {
        if self.call.as_ref().is_some_and(|call| call.channel == channel) {
            return;
        }
        let Some(space) = self.spaces.iter().find(|s| s.id == guild) else { return };
        let allowed = space.permissions(channel);
        if allowed & permissions::CONNECT == 0 {
            self.voice_notice(t!("voice.no_permission"), cx);
            return;
        }
        let limit = space.channels().iter().find(|c| c.id == channel).and_then(|c| c.user_limit).unwrap_or(0);
        let inside = space.voice_in(channel).count();
        if limit > 0 && inside >= limit as usize && allowed & permissions::MOVE_MEMBERS == 0 {
            self.voice_notice(t!("voice.full"), cx);
            return;
        }
        self.start_call(Some(guild), channel, false, cx);
    }

    /// Joins the call in `channel`, ringing the others in a DM when `ring`.
    fn start_call(&mut self, guild: Option<Snowflake>, channel: Snowflake, ring: bool, cx: &mut Context<Self>) {
        if self.call.is_some() {
            self.net.post(Command::VoiceDisconnect);
        }
        tracing::info!(dm = guild.is_none(), "voice: joining a call");
        self.call_notice = None;
        let mut call = Call::new(guild, channel);
        call.ring_on_create = ring;
        self.call = Some(call);
        self.post_voice_state();
        self.post_voice_controls();
        cx.notify();
    }

    /// Hangs up and tells Discord we left.
    pub(super) fn leave_voice(&mut self, cx: &mut Context<Self>) {
        let Some(call) = self.call.take() else { return };
        self.net.post(Command::VoiceDisconnect);
        self.net.post(Command::VoiceState { guild: call.guild, channel: None, self_mute: false, self_deaf: false });
        cx.notify();
    }

    /// Mutes or unmutes the microphone; unmuting also undeafens, as in
    /// the web client.
    pub(super) fn toggle_mute(&mut self, cx: &mut Context<Self>) {
        let voice = &mut self.settings.voice;
        if voice.deafened {
            voice.deafened = false;
            voice.muted = false;
        } else {
            voice.muted = !voice.muted;
        }
        self.voice_controls_changed(cx);
    }

    /// Deafening also mutes; undeafening restores the mute as it was.
    pub(super) fn toggle_deafen(&mut self, cx: &mut Context<Self>) {
        self.settings.voice.deafened = !self.settings.voice.deafened;
        self.voice_controls_changed(cx);
    }

    /// Saves the voice settings and passes them to the engine, and the
    /// mute to Discord when in a call.
    pub(super) fn voice_controls_changed(&mut self, cx: &mut Context<Self>) {
        self.post_voice_controls();
        if self.call.is_some() {
            self.post_voice_state();
        }
        self.save_settings(cx);
    }

    /// A volume changed: only the local engine hears of it. Discord's voice
    /// state carries mute and deafen, not volume, so nothing is sent, and a
    /// dragged slider saves once it stops.
    pub(in crate::app) fn voice_volume_changed(&mut self, cx: &mut Context<Self>) {
        self.post_voice_controls();
        self.save_settings_soon(cx);
        cx.notify();
    }

    /// What the engine is told: the saved switches, and muted while a
    /// stage keeps us in the audience.
    fn engine_controls(&self) -> VoiceControls {
        let mut controls = self.settings.voice.controls();
        controls.muted |= self.call.as_ref().is_some_and(|call| call.suppressed);
        controls
    }

    fn post_voice_controls(&self) {
        self.net.post(Command::VoiceControls(self.engine_controls()));
    }

    /// Op 4 for the call, with our mute and deafen.
    fn post_voice_state(&self) {
        let Some(call) = &self.call else { return };
        let voice = &self.settings.voice;
        self.net.post(Command::VoiceState {
            guild: call.guild,
            channel: Some(call.channel),
            self_mute: voice.muted || voice.deafened,
            self_deaf: voice.deafened,
        });
    }

    pub(super) fn on_voice_state(&mut self, state: VoiceState, cx: &mut Context<Self>) {
        if let Some(user) = state.member.as_ref().and_then(|m| m.user.as_ref()) {
            self.people.remember_new(user);
        }
        let mine = self.people.is_me(state.user_id);
        if let Some(guild) = state.guild_id {
            if let (Some(member), Some(info)) = (state.member.clone(), self.spaces.guild_mut(guild)) {
                info.add_member(state.user_id, member);
            }
            if let Some(space) = self.spaces.get_mut(guild) {
                space.set_voice_state(state.clone());
            }
        } else {
            self.dm_calls.voice_state(&state);
        }
        // Our own state answers a join; one from another device of ours, when
        // we asked for nothing, is only shown.
        if mine && let Some(call) = &mut self.call {
            let suppressed = call.suppressed;
            let own = call.own_state(&state);
            let suppression_changed = call.suppressed != suppressed;
            match own {
                Own::Joined => {
                    if suppression_changed {
                        self.post_voice_controls();
                    }
                    self.connect_voice();
                }
                Own::Left => {
                    tracing::info!("voice: removed from the channel");
                    self.call = None;
                    self.net.post(Command::VoiceDisconnect);
                }
            }
        }
        cx.notify();
    }

    pub(super) fn on_voice_server(&mut self, server: VoiceServer) {
        if let Some(call) = &mut self.call
            && call.set_server(server)
        {
            self.connect_voice();
        }
    }

    /// Starts the engine once the call has all it needs.
    fn connect_voice(&mut self) {
        let me = self.people.my_id();
        if let Some(details) = self.call.as_mut().and_then(|call| call.connect(me)) {
            self.net.post(Command::VoiceConnect(details));
        }
    }

    pub(super) fn on_voice_event(&mut self, event: VoiceEvent, cx: &mut Context<Self>) {
        let channel = match &event {
            VoiceEvent::Status { channel, .. }
            | VoiceEvent::Speaking { channel, .. }
            | VoiceEvent::MicrophoneUnavailable { channel }
            | VoiceEvent::Ended { channel, .. } => *channel,
        };
        // News of a call we already left.
        let Some(call) = self.call.as_mut().filter(|call| call.channel == channel) else { return };
        match event {
            VoiceEvent::Status { status, .. } => call.status = status,
            VoiceEvent::Speaking { users, .. } => call.speaking = users.into_iter().collect(),
            VoiceEvent::MicrophoneUnavailable { .. } => call.listen_only = true,
            VoiceEvent::Ended { error: None, .. } => {
                self.call = None;
            }
            VoiceEvent::Ended { error: Some(_), .. } => {
                // The engine logged why; Discord is told we are gone.
                self.leave_voice(cx);
                self.voice_notice(t!("voice.failed"), cx);
            }
        }
        cx.notify();
    }

    /// A new gateway session drops the old one's call on Discord's side.
    pub(super) fn end_call_for_new_session(&mut self) {
        if self.call.take().is_some() {
            self.net.post(Command::VoiceDisconnect);
        }
    }

    /// A short message in the call panel's place.
    fn voice_notice(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.call_notice = Some(text.into());
        cx.notify();
    }

    /// Whether `user` is speaking in our call.
    pub(super) fn is_speaking(&self, user: Snowflake) -> bool {
        self.call.as_ref().is_some_and(|call| call.speaking.contains(&user))
    }

    /// The words under "Voice Connected" in the call panel.
    pub(super) fn call_status_text(status: &CallStatus) -> SharedString {
        match status {
            CallStatus::Connecting => t!("voice.connecting").into(),
            CallStatus::Securing => t!("voice.securing").into(),
            CallStatus::Connected { .. } => t!("voice.connected").into(),
        }
    }
}
