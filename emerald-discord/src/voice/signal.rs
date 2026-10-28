//! Frames from the voice gateway: the handshake (hello, ready, session
//! description), who sends on which SSRC, and DAVE's group changes.

use std::net::IpAddr;
use std::time::Duration;

use anyhow::{Result, anyhow};
use serde_json::Value;
use tokio::time::Instant;

use super::session::Session;
use super::socket::Socket;
use super::transport::Transport;
use crate::packets::voice::{self as packets, op};

impl Session {
    pub async fn json(&mut self, frame: Value, socket: &mut Socket) -> Result<()> {
        if let Some(seq) = frame["seq"].as_i64() {
            self.seq_ack = seq;
        }
        let Some(opcode) = frame["op"].as_u64().and_then(|o| u8::try_from(o).ok()) else { return Ok(()) };
        let data = &frame["d"];
        if opcode >= op::DAVE_PREPARE_TRANSITION || opcode == op::CLIENTS_CONNECT || opcode == op::CLIENT_DISCONNECT {
            // Only the opcode: enough to see where a handshake stopped.
            tracing::info!(opcode, "voice: gateway frame");
        }
        match opcode {
            op::HELLO => {
                let interval = data["heartbeat_interval"].as_f64().filter(|ms| (100.0..=120_000.0).contains(ms));
                let interval = Duration::from_millis(interval.ok_or_else(|| anyhow!("bad voice heartbeat"))? as u64);
                // The web client beats at least every five seconds outside the desktop app.
                let interval = interval.min(Duration::from_secs(5));
                self.server_version = data["v"].as_u64().unwrap_or(3);
                self.heartbeat = Some(interval);
                self.next_heartbeat = Instant::now() + interval;
                self.awaiting_ack = false;
            }
            op::HEARTBEAT_ACK => self.awaiting_ack = false,
            op::READY => self.ready(data, socket).await?,
            op::SESSION_DESCRIPTION => self.described(data, socket).await?,
            op::RESUMED => tracing::info!("voice: resumed"),
            op::SPEAKING => {
                if let (Some(user), Some(ssrc)) = (user_id(&data["user_id"]), ssrc(&data["ssrc"])) {
                    self.announce(user, ssrc)?;
                }
            }
            // Client flags and media: the audio SSRC of someone joining.
            op::CLIENT_FLAGS => {
                if let (Some(user), Some(ssrc)) = (user_id(&data["user_id"]), ssrc(&data["audio_ssrc"])) {
                    self.announce(user, ssrc)?;
                }
            }
            op::CLIENTS_CONNECT => {
                let users = data["user_ids"].as_array().into_iter().flatten().filter_map(user_id);
                self.dave.connect(users);
            }
            op::CLIENT_DISCONNECT => {
                if let Some(user) = user_id(&data["user_id"]) {
                    if let (Some(ssrc), Some(transport)) = (self.mixer.ssrc(user), &mut self.transport) {
                        transport.forget(ssrc)?;
                    }
                    self.mixer.remove(user);
                    self.dave.disconnect(user);
                }
            }
            op::DAVE_PREPARE_TRANSITION => {
                let transition = number(&data["transition_id"]);
                if number(&data["protocol_version"]) == 0 {
                    tracing::info!("voice: the call is no longer end-to-end encrypted");
                    self.dave.set_version(0);
                }
                if transition != 0 {
                    socket.send(packets::transition_ready(transition)).await?;
                }
            }
            op::DAVE_EXECUTE_TRANSITION => self.report(),
            // Epoch 1 starts a new group, as after everyone else left.
            op::DAVE_PREPARE_EPOCH if number(&data["epoch"]) == 1 => {
                self.dave.set_version(number(&data["protocol_version"]));
                self.dave.reset()?;
                socket.send_binary(self.dave.key_package()?).await?;
            }
            _ => {}
        }
        Ok(())
    }

    pub async fn binary(&mut self, frame: &[u8], socket: &mut Socket) -> Result<()> {
        let Some((seq, opcode, body)) = packets::binary(frame) else { return Ok(()) };
        self.seq_ack = i64::from(seq);
        tracing::info!(opcode, "voice: DAVE frame");
        match opcode {
            op::DAVE_EXTERNAL_SENDER => self.dave.external_sender(body)?,
            op::DAVE_PROPOSALS => match self.dave.proposals(body) {
                Ok(Some(commit)) => socket.send_binary(commit).await?,
                Ok(None) => {}
                Err(e) => tracing::warn!("voice: {e:#}"),
            },
            op::DAVE_ANNOUNCE_COMMIT | op::DAVE_WELCOME => {
                let (transition, outcome) = self.dave.transition(opcode, body);
                match outcome {
                    Ok(()) if transition != 0 => socket.send(packets::transition_ready(transition)).await?,
                    Ok(()) => {}
                    Err(e) => {
                        // Ask to be added again, with a fresh group.
                        tracing::warn!("voice: {e:#}");
                        socket.send(packets::invalid_commit_welcome(transition)).await?;
                        self.dave.reset()?;
                        socket.send_binary(self.dave.key_package()?).await?;
                    }
                }
                self.report();
            }
            _ => {}
        }
        Ok(())
    }

    /// `user` sends on `ssrc`: their packets are let in and mixed.
    fn announce(&mut self, user: u64, ssrc: u32) -> Result<()> {
        if user == self.call.user.0 || ssrc == 0 {
            return Ok(());
        }
        if let Some(transport) = &mut self.transport {
            if let Some(old) = self.mixer.ssrc(user).filter(|old| *old != ssrc) {
                transport.forget(old)?;
            }
            transport.expect(ssrc)?;
        }
        self.mixer.announce(user, ssrc);
        Ok(())
    }

    /// Op 2: our SSRC and the voice server's address. The WebRTC offer
    /// goes out; the server's address comes back in its answer.
    async fn ready(&mut self, data: &Value, socket: &mut Socket) -> Result<()> {
        if self.transport.is_some() {
            return Ok(());
        }
        self.ssrc = ssrc(&data["ssrc"]).ok_or_else(|| anyhow!("voice ready without an SSRC"))?;
        let address: Option<IpAddr> = data["ip"].as_str().and_then(|ip| ip.parse().ok());
        if self.server_version >= 6 {
            socket.send(packets::backend_version()).await?;
        }
        let mut transport = Transport::new(self.ssrc, address.is_some_and(|a| a.is_ipv6())).await?;
        socket.send(packets::select_protocol(&transport.offer(), &self.connection_id)).await?;
        self.transport = Some(transport);
        Ok(())
    }

    /// Op 4: the voice server's SDP answer, and whether the call is
    /// end-to-end encrypted. The sound devices open now.
    async fn described(&mut self, data: &Value, socket: &mut Socket) -> Result<()> {
        let Some(transport) = &mut self.transport else { return Ok(()) };
        // A resumed gateway keeps the media path it had.
        if transport.connected() {
            return Ok(());
        }
        let sdp = data["sdp"].as_str().ok_or_else(|| anyhow!("the voice server sent no SDP"))?;
        transport.answer(sdp).await?;
        self.dave.set_version(number(&data["dave_protocol_version"]));
        if self.dave.encrypted() {
            socket.send_binary(self.dave.key_package()?).await?;
        }
        self.open_sound();
        self.report();
        tracing::info!(encrypted = self.dave.encrypted(), "voice: negotiated");
        Ok(())
    }
}

/// Snowflakes come as strings.
fn user_id(value: &Value) -> Option<u64> {
    value.as_str()?.parse().ok().filter(|id| *id != 0)
}

fn ssrc(value: &Value) -> Option<u32> {
    value.as_u64().and_then(|v| u32::try_from(v).ok())
}

fn number(value: &Value) -> u16 {
    value.as_u64().and_then(|v| u16::try_from(v).ok()).unwrap_or(0)
}
