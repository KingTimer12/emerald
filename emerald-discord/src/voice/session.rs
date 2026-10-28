//! One call from identify to hang-up: every 20 ms a tick captures, encodes
//! and sends our sound and plays everyone else's; between ticks, frames
//! from the voice gateway ([`super::signal`]) and datagrams from the voice
//! server ([`super::transport`]) come in.

use std::time::Duration;

use anyhow::{Result, anyhow};
use emerald_audio::{Audio, Choice, Denoiser, FRAME, Frame, Microphone, Speaker};
use opus2::{Application, Bitrate, Channels, Encoder};
use tokio::sync::watch;
use tokio::time::{Instant, MissedTickBehavior, interval};

use super::Events;
use super::dave::Dave;
use super::mixer::Mixer;
use super::socket::{Frame as SocketFrame, Socket};
use super::transport::{MAX_PACKET, Transport};
use crate::model::Snowflake;
use crate::packets::voice::{self as packets, Session as Credentials};
use crate::protocol::{CallStatus, NetEvent, VoiceCall, VoiceControls, VoiceEvent};

const TICK: Duration = Duration::from_millis(20);
/// A call that is not up by then gives up.
const SETUP_TIMEOUT: Duration = Duration::from_secs(30);
/// Frames still sent once the voice drops under the sensitivity: 300 ms,
/// so word endings are not clipped.
const HANGOVER: u8 = 15;
/// Silence frames sent before saying we stopped speaking.
const TRAILING_SILENCE: u8 = 5;
/// Captured frames sent in one tick; more means the tick fell behind.
const MAX_FRAMES_PER_TICK: usize = 3;
/// Voice gateway reconnects in a row before the call is given up.
const MAX_RESUMES: u8 = 3;
const BITRATE: i32 = 64_000;
/// Largest Opus packet.
const MAX_OPUS: usize = 1275;
/// Ticks between speaking reports to the UI: 100 ms.
const SPEAKING_EVERY: u32 = 5;

/// The open microphone and speakers.
struct Sound {
    audio: Audio,
    microphone: Microphone,
    speaker: Speaker,
}

pub(super) struct Session {
    pub call: VoiceCall,
    events: Events,
    pub controls: VoiceControls,
    pub transport: Option<Transport>,
    pub ssrc: u32,
    /// The voice gateway's version, from HELLO.
    pub server_version: u64,
    pub dave: Dave,
    pub mixer: Mixer,
    sound: Option<Sound>,
    encoder: Encoder,
    denoiser: Denoiser,
    /// The last numbered frame from the voice gateway.
    pub seq_ack: i64,
    pub heartbeat: Option<Duration>,
    pub next_heartbeat: Instant,
    pub awaiting_ack: bool,
    nonce: u64,
    timestamp: u32,
    hangover: u8,
    trailing: u8,
    speaking: bool,
    status: Option<CallStatus>,
    speakers: Vec<Snowflake>,
    ticks: u32,
    /// Names this connection to the voice server.
    pub connection_id: String,
}

pub async fn run(call: VoiceCall, mut controls: watch::Receiver<VoiceControls>, events: Events) -> Result<()> {
    let mut socket = Socket::open(&call.endpoint).await?;
    let mut session = Session::new(call, controls.borrow_and_update().clone(), events)?;
    session.report();
    socket.send(packets::identify(&session.credentials())).await?;
    tracing::info!("voice: connecting");

    let started = Instant::now();
    let mut tick = interval(TICK);
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut datagram = vec![0u8; MAX_PACKET];
    let mut resumes = 0;
    loop {
        let lost = tokio::select! {
            changed = controls.changed() => {
                // The UI's side lives as long as the network thread.
                changed?;
                let next = controls.borrow_and_update().clone();
                session.apply(next);
                false
            }
            _ = tick.tick() => {
                if !session.connected() && started.elapsed() > SETUP_TIMEOUT {
                    return Err(anyhow!("voice setup timed out"));
                }
                if session.heartbeat_missed() {
                    true
                } else {
                    session.tick(&mut socket).await?;
                    false
                }
            }
            received = receive(session.transport.as_ref(), &mut datagram) => {
                match received {
                    Ok(length) => session.datagram(&datagram[..length])?,
                    Err(e) if transient(&e) => {}
                    Err(e) => return Err(anyhow!("voice UDP: {e}")),
                }
                false
            }
            frame = socket.next() => match frame {
                Ok(SocketFrame::Json(frame)) => {
                    session.json(frame, &mut socket).await?;
                    false
                }
                Ok(SocketFrame::Binary(frame)) => {
                    session.binary(&frame, &mut socket).await?;
                    false
                }
                // Kicked, moved or the channel deleted: Discord says not to come back.
                Ok(SocketFrame::Closed(Some(4014 | 4022))) => return Ok(()),
                Ok(SocketFrame::Closed(Some(code @ (4004 | 4006 | 4009 | 4011 | 4016)))) => {
                    return Err(anyhow!("voice gateway closed with {code}"));
                }
                Ok(SocketFrame::Closed(code)) => {
                    tracing::info!(?code, "voice: gateway closed");
                    true
                }
                Err(e) => {
                    tracing::info!("voice: gateway failed: {e:#}");
                    true
                }
            },
        };
        if lost {
            // Only a call already up can be resumed.
            resumes += 1;
            if !session.media_connected() || resumes > MAX_RESUMES {
                return Err(anyhow!("voice gateway lost"));
            }
            tracing::info!(attempt = resumes, "voice: resuming");
            socket = Socket::open(&session.call.endpoint).await?;
            socket.send(packets::resume(&session.credentials(), session.seq_ack)).await?;
            session.heartbeat = None;
            session.awaiting_ack = false;
        } else if session.connected() && !session.awaiting_ack {
            resumes = 0;
        }
    }
}

/// A datagram, or never while there is no transport yet.
async fn receive(transport: Option<&Transport>, buffer: &mut [u8]) -> std::io::Result<usize> {
    match transport {
        Some(transport) => transport.socket().recv(buffer).await,
        None => std::future::pending().await,
    }
}

/// One lost or refused datagram, not a dead socket.
fn transient(error: &std::io::Error) -> bool {
    use std::io::ErrorKind::*;
    matches!(error.kind(), ConnectionReset | ConnectionRefused | Interrupted | WouldBlock)
}

impl Session {
    fn new(call: VoiceCall, controls: VoiceControls, events: Events) -> Result<Self> {
        let mut encoder = Encoder::new(emerald_audio::RATE, Channels::Mono, Application::Voip)?;
        encoder.set_bitrate(Bitrate::Bits(BITRATE))?;
        encoder.set_inband_fec(true)?;
        let dave = Dave::new(call.user.0, call.channel.0)?;
        let now = Instant::now();
        Ok(Self {
            call,
            events,
            controls,
            transport: None,
            ssrc: 0,
            server_version: 0,
            dave,
            mixer: Mixer::default(),
            sound: None,
            encoder,
            denoiser: Denoiser::default(),
            seq_ack: -1,
            heartbeat: None,
            next_heartbeat: now,
            awaiting_ack: false,
            nonce: 0,
            // A random starting point, as RTP asks.
            timestamp: rand_u32(),
            hangover: 0,
            trailing: 0,
            speaking: false,
            status: None,
            speakers: Vec::new(),
            ticks: 0,
            connection_id: uuid::Uuid::new_v4().to_string(),
        })
    }

    pub fn credentials(&self) -> Credentials<'_> {
        Credentials {
            server: self.call.guild.unwrap_or(self.call.channel),
            channel: self.call.channel,
            user: self.call.user,
            session_id: &self.call.session_id,
            token: &self.call.token,
        }
    }

    fn emit(&self, event: VoiceEvent) {
        self.events.try_send(NetEvent::Voice(event)).ok();
    }

    fn connected(&self) -> bool {
        matches!(self.status, Some(CallStatus::Connected { .. }))
    }

    /// DTLS is done: sound can flow.
    fn media_connected(&self) -> bool {
        self.transport.as_ref().is_some_and(Transport::connected)
    }

    /// Tells the UI when the call moved along.
    pub fn report(&mut self) {
        let status = if !self.media_connected() {
            CallStatus::Connecting
        } else if !self.dave.ready() {
            CallStatus::Securing
        } else {
            CallStatus::Connected { privacy_code: self.dave.encrypted().then(|| self.dave.privacy_code()).flatten() }
        };
        if self.status.as_ref() != Some(&status) {
            tracing::info!(status = ?std::mem::discriminant(&status), "voice: status");
            self.status = Some(status.clone());
            self.emit(VoiceEvent::Status { channel: self.call.channel, status });
        }
    }

    /// The UI changed mute, deafen, devices or volumes.
    fn apply(&mut self, controls: VoiceControls) {
        let devices_changed = controls.input != self.controls.input || controls.output != self.controls.output;
        if controls.deafened && !self.controls.deafened {
            self.mixer.clear();
        }
        self.controls = controls;
        if devices_changed && self.sound.is_some() {
            self.open_sound();
        } else {
            self.set_switches();
        }
    }

    fn set_switches(&self) {
        let Some(sound) = &self.sound else { return };
        sound.audio.set_muted(self.controls.muted || self.controls.deafened);
        sound.audio.set_deafened(self.controls.deafened);
        sound.audio.set_volumes(self.controls.input_volume, self.controls.output_volume);
    }

    /// Opens (or reopens) the microphone and speakers.
    pub fn open_sound(&mut self) {
        self.sound = None;
        let choice = Choice { input: self.controls.input.clone(), output: self.controls.output.clone() };
        match Audio::open(&choice) {
            Ok((audio, microphone, speaker)) => {
                if audio.microphone_failed() {
                    self.emit(VoiceEvent::MicrophoneUnavailable { channel: self.call.channel });
                }
                self.sound = Some(Sound { audio, microphone, speaker });
                self.set_switches();
            }
            Err(e) => tracing::warn!("voice: no sound devices: {e:#}"),
        }
    }

    /// A beat went unanswered since the last one: the socket is dead.
    fn heartbeat_missed(&self) -> bool {
        self.heartbeat.is_some() && self.awaiting_ack && Instant::now() >= self.next_heartbeat
    }

    async fn tick(&mut self, socket: &mut Socket) -> Result<()> {
        self.ticks = self.ticks.wrapping_add(1);
        let now = Instant::now();
        if let Some(every) = self.heartbeat
            && now >= self.next_heartbeat
        {
            self.nonce += 1;
            socket.send(packets::heartbeat(self.nonce, self.seq_ack)).await?;
            self.awaiting_ack = true;
            self.next_heartbeat = now + every;
        }
        if let Some(transport) = &mut self.transport {
            transport.tick()?;
            if transport.failed() {
                return Err(anyhow!("voice media path lost"));
            }
        }
        if self.sound.as_ref().is_some_and(|s| s.audio.failed()) {
            tracing::info!("voice: a sound device failed, reopening");
            self.open_sound();
        }
        self.send_sound(socket).await?;
        self.play();
        self.report();
        if self.ticks.is_multiple_of(SPEAKING_EVERY) {
            self.report_speakers();
        }
        Ok(())
    }

    /// Sends what the microphone caught, while it is loud enough.
    async fn send_sound(&mut self, socket: &mut Socket) -> Result<()> {
        let live = self.media_connected() && self.dave.ready() && !self.controls.muted && !self.controls.deafened;
        let mut frames: Vec<Frame> = Vec::new();
        if let Some(sound) = &mut self.sound {
            while let Ok(frame) = sound.microphone.pop() {
                // A late tick finds a backlog; only the newest frames go out.
                if frames.len() == MAX_FRAMES_PER_TICK {
                    frames.remove(0);
                }
                frames.push(frame);
            }
        }
        if frames.is_empty() {
            // Time passes for the receivers even when nothing was captured.
            frames.push([0.; FRAME]);
        }
        for mut frame in frames {
            let mut level = f32::NEG_INFINITY;
            if live {
                if self.controls.noise_suppression {
                    self.denoiser.process(&mut frame);
                }
                level = emerald_audio::level(&frame);
                if level > f32::from(self.controls.sensitivity) {
                    self.hangover = HANGOVER;
                }
            } else {
                self.hangover = 0;
            }
            if self.hangover > 0 {
                self.hangover -= 1;
                let mut opus = [0u8; MAX_OPUS];
                let length = self.encoder.encode_float(&frame, &mut opus)?;
                let start = !self.speaking;
                if start {
                    socket.send(packets::speaking(true, self.ssrc)).await?;
                    self.speaking = true;
                }
                self.trailing = TRAILING_SILENCE;
                if let Some(sealed) = self.dave.encrypt(&opus[..length]).map(|c| c.into_owned()) {
                    // The audio level extension counts down from 0 dBov.
                    let level = (-level).clamp(0., 127.) as u8;
                    self.send_rtp(&sealed, start, level)?;
                }
            } else if self.speaking {
                // Silence frames tell decoders the voice stopped, not got lost.
                self.send_rtp(&davey::OPUS_SILENCE_PACKET, false, 127)?;
                self.trailing = self.trailing.saturating_sub(1);
                if self.trailing == 0 {
                    socket.send(packets::speaking(false, self.ssrc)).await?;
                    self.speaking = false;
                }
            }
            self.timestamp = self.timestamp.wrapping_add(FRAME as u32);
        }
        Ok(())
    }

    fn send_rtp(&mut self, payload: &[u8], start: bool, level: u8) -> Result<()> {
        match &mut self.transport {
            Some(transport) => transport.send(self.timestamp, payload, start, level),
            None => Ok(()),
        }
    }

    /// Plays one frame of everyone else.
    fn play(&mut self) {
        let mixed = self.mixer.mix();
        if self.controls.deafened {
            return;
        }
        if let (Some(frame), Some(sound)) = (mixed, &mut self.sound) {
            // A full ring means the speaker runs slow; the frame is dropped.
            sound.speaker.push(frame).ok();
        }
    }

    fn report_speakers(&mut self) {
        let mut speakers: Vec<Snowflake> = self.mixer.speaking().map(Snowflake).collect();
        if self.speaking && self.hangover > 0 {
            speakers.push(self.call.user);
        }
        speakers.sort_unstable();
        if speakers != self.speakers {
            self.speakers = speakers.clone();
            self.emit(VoiceEvent::Speaking { channel: self.call.channel, users: speakers });
        }
    }

    /// A datagram from the voice server: ICE, DTLS, RTCP or sound.
    fn datagram(&mut self, datagram: &[u8]) -> Result<()> {
        let Some(transport) = &mut self.transport else { return Ok(()) };
        transport.receive(datagram)?;
        let received = transport.take_received();
        if self.controls.deafened {
            return Ok(());
        }
        for packet in received {
            let Some(user) = self.mixer.user(packet.ssrc) else { continue };
            if let Some(opus) = self.dave.decrypt(user, packet.payload) {
                self.mixer.push(packet.ssrc, packet.sequence, opus);
            }
        }
        Ok(())
    }
}

/// Not secret, only unpredictable enough for RTP's starting points.
pub(super) fn rand_u32() -> u32 {
    let bytes = *uuid::Uuid::new_v4().as_bytes();
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}
