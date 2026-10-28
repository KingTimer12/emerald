//! The media path the web client uses: WebRTC, through str0m. Discord's
//! media servers are ICE-lite, so we are the controlling agent, and the
//! DTLS client, as a browser that made the offer is. SRTP carries the Opus
//! packets, which are still DAVE ciphertext in an end-to-end encrypted call.
//!
//! str0m does no I/O and keeps no clock: every input (a datagram, a tick)
//! is followed by draining its output (datagrams to send, events) before
//! the next one, as its API requires.

use std::net::SocketAddr;
use std::time::Instant;

use anyhow::{Context, Result, anyhow};
use str0m::config::Fingerprint;
use str0m::media::{MediaKind, Mid, Pt};
use str0m::net::{Protocol, Receive};
use str0m::rtp::{Extension, ExtensionValues, RtpWrite, Ssrc};
use str0m::{Candidate, Event, IceConnectionState, IceCreds, Input, Output, Rtc, RtcConfig};
use tokio::net::UdpSocket;

use crate::packets::voice::{self as packets, AUDIO_EXTENSIONS, OPUS_PAYLOAD_TYPE};

/// Larger datagrams are not voice.
pub const MAX_PACKET: usize = 2000;
/// Received packets kept between two drains; more are dropped.
const MAX_RECEIVED: usize = 64;

/// A packet from someone else, out of SRTP.
pub struct Received {
    pub ssrc: u32,
    pub sequence: u16,
    pub payload: Vec<u8>,
}

pub struct Transport {
    rtc: Rtc,
    udp: UdpSocket,
    /// Our address and the voice server's, once the answer is in.
    route: Option<(SocketAddr, SocketAddr)>,
    mid: Mid,
    ssrc: Ssrc,
    /// The next RTP sequence number.
    sequence: u64,
    connected: bool,
    failed: bool,
    received: Vec<Received>,
}

impl Transport {
    /// A transport sending on `ssrc`, the one READY gave us.
    pub async fn new(ssrc: u32, ipv6: bool) -> Result<Self> {
        let mut config = RtcConfig::new()
            .clear_codecs()
            .enable_opus(true, false)
            .set_rtp_mode(true)
            .set_stats_interval(None)
            .set_local_ice_credentials(chrome_credentials())
            .clear_extension_map();
        for (id, uri) in AUDIO_EXTENSIONS {
            config = config.set_extension(id, extension(uri));
        }
        let mut rtc = config.build(Instant::now());
        let mid = Mid::from("0");
        let mut api = rtc.direct_api();
        api.set_ice_controlling(true);
        api.declare_media(mid, MediaKind::Audio);
        api.declare_stream_tx(Ssrc::from(ssrc), None, mid, None);
        let udp = UdpSocket::bind(if ipv6 { "[::]:0" } else { "0.0.0.0:0" }).await?;
        let mut transport = Self {
            rtc,
            udp,
            route: None,
            mid,
            ssrc: Ssrc::from(ssrc),
            // A random start, as RTP asks; under 2^16 so no rollover needs signalling.
            sequence: u64::from(super::session::rand_u32() as u16),
            connected: false,
            failed: false,
            received: Vec::new(),
        };
        transport.drain()?;
        Ok(transport)
    }

    /// The offer for op 1.
    pub fn offer(&mut self) -> String {
        let api = self.rtc.direct_api();
        let credentials = api.local_ice_credentials();
        packets::offer(&credentials.ufrag, &credentials.pass, &api.local_dtls_fingerprint().to_string())
    }

    /// The voice server's answer from op 4: ICE and DTLS start.
    pub async fn answer(&mut self, sdp: &str) -> Result<()> {
        let answer = packets::answer(sdp).ok_or_else(|| anyhow!("the voice server's SDP is incomplete"))?;
        let fingerprint: Fingerprint = answer.fingerprint.parse().map_err(|e| anyhow!("voice server fingerprint: {e}"))?;
        // Connecting picks the interface that reaches the server: its address is our candidate.
        self.udp.connect(answer.address).await?;
        let local = self.udp.local_addr()?;
        self.route = Some((local, answer.address));
        self.rtc.add_local_candidate(Candidate::host(local, "udp").context("local candidate")?);
        self.drain()?;
        self.rtc.add_remote_candidate(Candidate::host(answer.address, "udp").context("voice server candidate")?);
        self.drain()?;
        let mut api = self.rtc.direct_api();
        api.set_remote_ice_credentials(IceCreds { ufrag: answer.ufrag, pass: answer.password });
        api.set_remote_fingerprint(fingerprint);
        api.start_dtls(true)?;
        self.drain()
    }

    /// DTLS is done and SRTP keyed.
    pub fn connected(&self) -> bool {
        self.connected
    }

    /// ICE gave up on the voice server.
    pub fn failed(&self) -> bool {
        self.failed
    }

    pub fn socket(&self) -> &UdpSocket {
        &self.udp
    }

    /// Lets `ssrc`'s packets in; str0m drops those of unknown streams.
    pub fn expect(&mut self, ssrc: u32) -> Result<()> {
        let ssrc = Ssrc::from(ssrc);
        let mut api = self.rtc.direct_api();
        if api.stream_rx(&ssrc).is_none() {
            // Chrome asks again for lost audio only when told to; the answer the web client builds does not.
            api.expect_stream_rx(ssrc, None, self.mid, None).suppress_nack(true);
        }
        self.drain()
    }

    pub fn forget(&mut self, ssrc: u32) -> Result<()> {
        self.rtc.direct_api().remove_stream_rx(Ssrc::from(ssrc));
        self.drain()
    }

    /// Time passed: retransmissions, ICE consent checks, RTCP reports.
    pub fn tick(&mut self) -> Result<()> {
        self.rtc.handle_input(Input::Timeout(Instant::now()))?;
        self.drain()
    }

    /// A datagram from the voice server.
    pub fn receive(&mut self, datagram: &[u8]) -> Result<()> {
        let Some((local, server)) = self.route else { return Ok(()) };
        let Ok(contents) = datagram.try_into() else { return Ok(()) };
        let input = Input::Receive(Instant::now(), Receive { proto: Protocol::Udp, source: server, destination: local, contents });
        if let Err(e) = self.rtc.handle_input(input) {
            // One bad datagram is not a dead call.
            tracing::debug!("voice: datagram refused: {e}");
        }
        self.drain()
    }

    /// Packets received since the last call.
    pub fn take_received(&mut self) -> Vec<Received> {
        std::mem::take(&mut self.received)
    }

    /// One Opus packet, `time` in 48 kHz units. `start` marks the first
    /// packet after silence; `level` is its loudness in -dBov, as Chrome
    /// puts in the audio level extension.
    pub fn send(&mut self, time: u32, payload: &[u8], start: bool, level: u8) -> Result<()> {
        if !self.connected {
            return Ok(());
        }
        let values = ExtensionValues {
            audio_level: Some(-(level.min(127) as i8)),
            voice_activity: Some(true),
            ..Default::default()
        };
        let packet = RtpWrite::new(Pt::from(OPUS_PAYLOAD_TYPE), self.sequence.into(), time, Instant::now(), payload.to_vec())
            .marker(start)
            .ext_vals(values);
        let mut api = self.rtc.direct_api();
        let Some(stream) = api.stream_tx(&self.ssrc) else { return Ok(()) };
        stream.write_rtp(packet);
        self.sequence += 1;
        self.drain()
    }

    /// Sends what str0m wants sent and keeps what it hands over, until it
    /// only has a deadline left.
    fn drain(&mut self) -> Result<()> {
        loop {
            match self.rtc.poll_output()? {
                Output::Timeout(_) => return Ok(()),
                // A full send buffer drops the datagram rather than stall the tick.
                Output::Transmit(transmit) => _ = self.udp.try_send(&transmit.contents),
                Output::Event(event) => self.event(event),
            }
        }
    }

    fn event(&mut self, event: Event) {
        match event {
            Event::Connected => {
                tracing::info!("voice: media connected");
                self.connected = true;
            }
            Event::IceConnectionStateChange(IceConnectionState::Disconnected) => self.failed = true,
            // Padding-only packets (the server probing bandwidth) carry no sound.
            Event::RtpPacket(packet) if !packet.payload.is_empty() && self.received.len() < MAX_RECEIVED => self.received.push(Received {
                ssrc: *packet.header.ssrc,
                sequence: packet.header.sequence_number,
                payload: packet.payload.to_vec(),
            }),
            _ => {}
        }
    }
}

fn extension(uri: &str) -> Extension {
    match uri {
        "urn:ietf:params:rtp-hdrext:ssrc-audio-level" => Extension::AudioLevel,
        "http://www.webrtc.org/experiments/rtp-hdrext/abs-send-time" => Extension::AbsoluteSendTime,
        "urn:ietf:params:rtp-hdrext:sdes:mid" => Extension::RtpMid,
        // The transport-wide congestion control sequence number.
        _ => Extension::TransportSequenceNumber,
    }
}

/// ICE credentials shaped like Chrome's: a 4-character username fragment
/// and a 24-character password, from the ICE character set.
fn chrome_credentials() -> IceCreds {
    const CHARACTERS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let random: Vec<u8> = [uuid::Uuid::new_v4(), uuid::Uuid::new_v4()].iter().flat_map(|id| *id.as_bytes()).collect();
    let text = |bytes: &[u8]| bytes.iter().map(|b| char::from(CHARACTERS[usize::from(b & 63)])).collect::<String>();
    IceCreds { ufrag: text(&random[..4]), pass: text(&random[4..28]) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_look_like_chrome_ones() {
        let credentials = chrome_credentials();
        assert_eq!((credentials.ufrag.len(), credentials.pass.len()), (4, 24));
        assert_ne!(credentials.pass, chrome_credentials().pass);
    }

    #[tokio::test]
    async fn offer_carries_our_credentials_and_certificate() {
        let mut transport = Transport::new(7, false).await.unwrap();
        let offer = transport.offer();
        let answer_like = offer.replace("a=ice-options:trickle", "a=candidate:1 1 UDP 1 127.0.0.1 9 typ host");
        let parsed = packets::answer(&answer_like).unwrap();
        assert_eq!(parsed.ufrag.len(), 4);
        assert!(parsed.fingerprint.starts_with("sha-256 "));
        assert!(!transport.connected());
    }

    /// A stand-in for Discord's media server: ICE-lite, DTLS server.
    struct Server {
        rtc: Rtc,
        udp: UdpSocket,
        address: SocketAddr,
        connected: bool,
        received: Vec<(u32, Vec<u8>)>,
    }

    impl Server {
        async fn new(offer: &str) -> Self {
            let udp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let address = udp.local_addr().unwrap();
            let mut rtc = RtcConfig::new().clear_codecs().enable_opus(true, false).set_rtp_mode(true).set_ice_lite(true).build(Instant::now());
            rtc.add_local_candidate(Candidate::host(address, "udp").unwrap());
            let ours = packets::answer(&offer.replace("a=ice-options:trickle", "a=candidate:1 1 UDP 1 127.0.0.1 9 typ host")).unwrap();
            let mut api = rtc.direct_api();
            api.set_ice_controlling(false);
            api.set_remote_ice_credentials(IceCreds { ufrag: ours.ufrag, pass: ours.password });
            api.set_remote_fingerprint(ours.fingerprint.parse().unwrap());
            let mid = Mid::from("0");
            api.declare_media(mid, MediaKind::Audio);
            api.expect_stream_rx(Ssrc::from(7), None, mid, None);
            api.declare_stream_tx(Ssrc::from(9), None, mid, None);
            api.start_dtls(false).unwrap();
            let mut server = Self { rtc, udp, address, connected: false, received: Vec::new() };
            server.drain();
            server
        }

        fn answer(&mut self) -> String {
            let api = self.rtc.direct_api();
            let credentials = api.local_ice_credentials();
            format!(
                "m=audio {port} ICE/SDP\na=fingerprint:{fingerprint}\nc=IN IP4 127.0.0.1\na=ice-ufrag:{ufrag}\na=ice-pwd:{pass}\na=candidate:1 1 UDP 4261412862 127.0.0.1 {port} typ host\n",
                port = self.address.port(),
                fingerprint = api.local_dtls_fingerprint(),
                ufrag = credentials.ufrag,
                pass = credentials.pass,
            )
        }

        fn drain(&mut self) {
            loop {
                match self.rtc.poll_output().unwrap() {
                    Output::Timeout(_) => return,
                    Output::Transmit(t) => _ = self.udp.try_send_to(&t.contents, t.destination),
                    Output::Event(Event::Connected) => self.connected = true,
                    Output::Event(Event::RtpPacket(p)) => self.received.push((*p.header.ssrc, p.payload.to_vec())),
                    Output::Event(_) => {}
                }
            }
        }

        fn receive(&mut self, datagram: &[u8], source: SocketAddr) {
            let input = Input::Receive(
                Instant::now(),
                Receive { proto: Protocol::Udp, source, destination: self.address, contents: datagram.try_into().unwrap() },
            );
            self.rtc.handle_input(input).unwrap();
            self.drain();
        }

        fn send(&mut self, sequence: u64, payload: &[u8]) {
            let packet = RtpWrite::new(Pt::from(OPUS_PAYLOAD_TYPE), sequence.into(), 960, Instant::now(), payload.to_vec());
            self.rtc.direct_api().stream_tx(&Ssrc::from(9)).unwrap().write_rtp(packet);
            self.drain();
        }
    }

    /// Runs both ends for a while, until `done`.
    async fn pump(client: &mut Transport, server: &mut Server, done: impl Fn(&Transport, &Server) -> bool) {
        let mut buffer = vec![0u8; MAX_PACKET];
        let mut server_buffer = vec![0u8; MAX_PACKET];
        for _ in 0..500 {
            if done(client, server) {
                return;
            }
            tokio::select! {
                Ok(length) = client.socket().recv(&mut buffer) => client.receive(&buffer[..length]).unwrap(),
                Ok((length, source)) = server.udp.recv_from(&mut server_buffer) => server.receive(&server_buffer[..length], source),
                _ = tokio::time::sleep(std::time::Duration::from_millis(10)) => {
                    client.tick().unwrap();
                    server.rtc.handle_input(Input::Timeout(Instant::now())).unwrap();
                    server.drain();
                }
            }
        }
        panic!("the two ends never got there");
    }

    #[tokio::test]
    async fn talks_to_an_ice_lite_server_over_dtls_and_srtp() {
        let mut client = Transport::new(7, false).await.unwrap();
        let mut server = Server::new(&client.offer()).await;
        client.answer(&server.answer()).await.unwrap();
        pump(&mut client, &mut server, |c, s| c.connected() && s.connected).await;

        client.send(1000, b"opus from us", true, 30).unwrap();
        pump(&mut client, &mut server, |_, s| !s.received.is_empty()).await;
        assert_eq!(server.received[0], (7, b"opus from us".to_vec()));

        client.expect(9).unwrap();
        server.send(500, b"opus from them");
        pump(&mut client, &mut server, |c, _| !c.received.is_empty()).await;
        let received = client.take_received();
        assert_eq!((received[0].ssrc, received[0].sequence, received[0].payload.as_slice()), (9, 500, &b"opus from them"[..]));
        assert!(!client.failed());
    }
}
