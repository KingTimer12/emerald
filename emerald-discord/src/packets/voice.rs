//! Voice server frames: JSON on the voice gateway (version 9, as the web
//! client asks for), DAVE's binary frames, and the SDP the web client
//! trades with the voice server for WebRTC. Field order follows the web
//! client's.

use std::net::SocketAddr;

use serde_json::{Value, json};

use crate::model::Snowflake;

/// Voice gateway version, as the web client requests it when its build
/// has no SFU update experiment.
pub const VERSION: u8 = 9;

/// Highest DAVE protocol version spoken here.
pub const DAVE_VERSION: u8 = 1;

/// Opus, as Chrome numbers it in its offer.
pub const OPUS_PAYLOAD_TYPE: u8 = 111;

/// Voice gateway opcodes; 21 and up are DAVE's.
pub mod op {
    pub const IDENTIFY: u8 = 0;
    pub const SELECT_PROTOCOL: u8 = 1;
    pub const READY: u8 = 2;
    pub const HEARTBEAT: u8 = 3;
    pub const SESSION_DESCRIPTION: u8 = 4;
    pub const SPEAKING: u8 = 5;
    pub const HEARTBEAT_ACK: u8 = 6;
    pub const RESUME: u8 = 7;
    pub const HELLO: u8 = 8;
    pub const RESUMED: u8 = 9;
    pub const CLIENTS_CONNECT: u8 = 11;
    pub const CLIENT_FLAGS: u8 = 12;
    pub const CLIENT_DISCONNECT: u8 = 13;
    pub const VOICE_BACKEND_VERSION: u8 = 16;
    pub const DAVE_PREPARE_TRANSITION: u8 = 21;
    pub const DAVE_EXECUTE_TRANSITION: u8 = 22;
    pub const DAVE_TRANSITION_READY: u8 = 23;
    pub const DAVE_PREPARE_EPOCH: u8 = 24;
    pub const DAVE_EXTERNAL_SENDER: u8 = 25;
    pub const DAVE_KEY_PACKAGE: u8 = 26;
    pub const DAVE_PROPOSALS: u8 = 27;
    pub const DAVE_COMMIT_WELCOME: u8 = 28;
    pub const DAVE_ANNOUNCE_COMMIT: u8 = 29;
    pub const DAVE_WELCOME: u8 = 30;
    pub const DAVE_INVALID_COMMIT_WELCOME: u8 = 31;
}

/// The voice server a call talks to: `server` is the guild, or the DM
/// channel for calls outside servers.
pub struct Session<'a> {
    pub server: Snowflake,
    pub channel: Snowflake,
    pub user: Snowflake,
    pub session_id: &'a str,
    pub token: &'a str,
}

/// Op 0. No `streams` while no video is sent, as when the web client has
/// no camera parameters (a guess: they are left out, not sent empty).
pub fn identify(session: &Session) -> Value {
    json!({
        "op": op::IDENTIFY,
        "d": {
            "server_id": session.server.to_string(),
            "channel_id": session.channel.to_string(),
            "user_id": session.user.to_string(),
            "session_id": session.session_id,
            "token": session.token,
            "max_dave_protocol_version": DAVE_VERSION,
            "video": false,
        },
    })
}

/// Op 7, picking a dropped connection up after the last numbered frame.
pub fn resume(session: &Session, seq_ack: i64) -> Value {
    json!({
        "op": op::RESUME,
        "d": {
            "token": session.token,
            "session_id": session.session_id,
            "server_id": session.server.to_string(),
            "channel_id": session.channel.to_string(),
            "seq_ack": seq_ack,
        },
    })
}

/// Op 1 for WebRTC: the offer goes both as `data` and as `sdp`, beside
/// the codecs it lists, as the web client sends it.
pub fn select_protocol(sdp: &str, connection: &str) -> Value {
    json!({
        "op": op::SELECT_PROTOCOL,
        "d": {
            "protocol": "webrtc",
            "data": sdp,
            "sdp": sdp,
            "codecs": codecs(),
            "rtc_connection_id": connection,
        },
    })
}

/// The codecs the web client reads out of Chrome's offer: Opus, then the
/// video codecs in its order of preference. Chrome's H264 and VP9 payload
/// types are a guess from its current offers; VP8 and Opus are fixed.
fn codecs() -> Value {
    json!([
        { "name": "opus", "type": "audio", "priority": 1000, "payload_type": OPUS_PAYLOAD_TYPE, "rtx_payload_type": null },
        { "name": "H264", "type": "video", "priority": 1000, "payload_type": 103, "rtx_payload_type": 104 },
        { "name": "VP8", "type": "video", "priority": 2000, "payload_type": 96, "rtx_payload_type": 97 },
        { "name": "VP9", "type": "video", "priority": 3000, "payload_type": 98, "rtx_payload_type": 99 },
    ])
}

/// Header extensions in Chrome's offer, audio's first; the ids are
/// Chrome's, so the voice server reads ours the way it reads a browser's.
pub const AUDIO_EXTENSIONS: [(u8, &str); 4] = [
    (1, "urn:ietf:params:rtp-hdrext:ssrc-audio-level"),
    (2, "http://www.webrtc.org/experiments/rtp-hdrext/abs-send-time"),
    (3, "http://www.ietf.org/id/draft-holmer-rmcat-transport-wide-cc-extensions-01"),
    (4, "urn:ietf:params:rtp-hdrext:sdes:mid"),
];
const VIDEO_EXTENSIONS: [(u8, &str); 8] = [
    (14, "urn:ietf:params:rtp-hdrext:toffset"),
    (13, "urn:3gpp:video-orientation"),
    (5, "http://www.webrtc.org/experiments/rtp-hdrext/playout-delay"),
    (6, "http://www.webrtc.org/experiments/rtp-hdrext/video-content-type"),
    (7, "http://www.webrtc.org/experiments/rtp-hdrext/video-timing"),
    (8, "http://www.webrtc.org/experiments/rtp-hdrext/color-space"),
    (10, "urn:ietf:params:rtp-hdrext:sdes:rtp-stream-id"),
    (11, "urn:ietf:params:rtp-hdrext:sdes:repaired-rtp-stream-id"),
];

/// Our offer as the web client sends it: Chrome's offer cut down to the
/// lines matching `^a=ice|a=extmap|a=fingerprint|opus|VP8|<rtx> rtx`,
/// each once, joined by `\n`. `fingerprint` is `sha-256 AB:CD:…`.
pub fn offer(ufrag: &str, password: &str, fingerprint: &str) -> String {
    let mut lines = vec![
        "a=extmap-allow-mixed".to_owned(),
        format!("a=ice-ufrag:{ufrag}"),
        format!("a=ice-pwd:{password}"),
        "a=ice-options:trickle".to_owned(),
        format!("a=fingerprint:{fingerprint}"),
    ];
    let extension = |(id, uri): &(u8, &str)| format!("a=extmap:{id} {uri}");
    lines.extend(AUDIO_EXTENSIONS.iter().map(extension));
    lines.push(format!("a=rtpmap:{OPUS_PAYLOAD_TYPE} opus/48000/2"));
    lines.extend(VIDEO_EXTENSIONS.iter().map(extension));
    lines.push("a=rtpmap:96 VP8/90000".to_owned());
    lines.push("a=rtpmap:97 rtx/90000".to_owned());
    lines.join("\n")
}

/// What the voice server's SDP answer tells us: its ICE credentials, its
/// DTLS certificate and where to send.
#[derive(Debug, PartialEq, Eq)]
pub struct Answer {
    pub ufrag: String,
    pub password: String,
    /// `sha-256 AB:CD:…`
    pub fingerprint: String,
    pub address: SocketAddr,
}

/// Reads the `sdp` of op 4. Like the web client, refuses one without a
/// fingerprint, ICE credentials or a candidate.
pub fn answer(sdp: &str) -> Option<Answer> {
    let attribute = |name: &str| sdp.lines().find_map(|line| line.trim().strip_prefix(name)).map(str::to_owned);
    let candidate = attribute("a=candidate:")?;
    // foundation component transport priority address port typ host
    let fields: Vec<&str> = candidate.split_whitespace().collect();
    if !fields.get(2)?.eq_ignore_ascii_case("udp") {
        return None;
    }
    let address = SocketAddr::new(fields.get(4)?.parse().ok()?, fields.get(5)?.parse().ok()?);
    Some(Answer {
        ufrag: attribute("a=ice-ufrag:")?,
        password: attribute("a=ice-pwd:")?,
        fingerprint: attribute("a=fingerprint:")?,
        address,
    })
}

/// Op 16, which the web client sends after READY to servers from version
/// 6 on; the answer names the voice server's build.
pub fn backend_version() -> Value {
    json!({ "op": op::VOICE_BACKEND_VERSION, "d": {} })
}

/// Op 3: `nonce` comes back in the ACK; `seq_ack` is the last numbered
/// frame received, -1 before any.
pub fn heartbeat(nonce: u64, seq_ack: i64) -> Value {
    json!({ "op": op::HEARTBEAT, "d": { "t": nonce, "seq_ack": seq_ack } })
}

/// Op 5: whether we are sending sound on `ssrc` (bit 1 is the microphone).
pub fn speaking(on: bool, ssrc: u32) -> Value {
    json!({ "op": op::SPEAKING, "d": { "speaking": u8::from(on), "delay": 0, "ssrc": ssrc } })
}

/// Op 23: ready for the transition Discord announced.
pub fn transition_ready(transition: u16) -> Value {
    json!({ "op": op::DAVE_TRANSITION_READY, "d": { "transition_id": transition } })
}

/// Op 31: the commit or welcome of `transition` could not be used.
pub fn invalid_commit_welcome(transition: u16) -> Value {
    json!({ "op": op::DAVE_INVALID_COMMIT_WELCOME, "d": { "transition_id": transition } })
}

/// Binary op 26: our MLS key package.
pub fn key_package(package: &[u8]) -> Vec<u8> {
    [&[op::DAVE_KEY_PACKAGE], package].concat()
}

/// Binary op 28: our commit, and the welcome for those it adds.
pub fn commit_welcome(commit: &[u8], welcome: Option<&[u8]>) -> Vec<u8> {
    [&[op::DAVE_COMMIT_WELCOME], commit, welcome.unwrap_or_default()].concat()
}

/// A binary frame from the server: its sequence number, opcode and body.
pub fn binary(frame: &[u8]) -> Option<(u16, u8, &[u8])> {
    let (&[high, low, opcode], body) = frame.split_first_chunk::<3>()?;
    Some((u16::from_be_bytes([high, low]), opcode, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session<'static> {
        Session { server: Snowflake(1), channel: Snowflake(2), user: Snowflake(3), session_id: "s", token: "t" }
    }

    #[test]
    fn identify_keeps_the_web_client_order() {
        assert_eq!(
            identify(&session()).to_string(),
            r#"{"op":0,"d":{"server_id":"1","channel_id":"2","user_id":"3","session_id":"s","token":"t","max_dave_protocol_version":1,"video":false}}"#
        );
    }

    #[test]
    fn select_protocol_sends_the_offer_twice() {
        let frame = select_protocol("a=ice-ufrag:x", "id");
        assert_eq!(frame["d"]["protocol"], "webrtc");
        assert_eq!(frame["d"]["data"], "a=ice-ufrag:x");
        assert_eq!(frame["d"]["sdp"], "a=ice-ufrag:x");
        assert_eq!(frame["d"]["codecs"][0]["payload_type"], 111);
        assert!(frame["d"]["codecs"][0]["rtx_payload_type"].is_null());
        let keys: Vec<&String> = frame["d"].as_object().unwrap().keys().collect();
        assert_eq!(keys, ["protocol", "data", "sdp", "codecs", "rtc_connection_id"]);
    }

    #[test]
    fn offer_has_the_lines_the_web_client_keeps() {
        let sdp = offer("abcd", "p".repeat(24).as_str(), "sha-256 AA:BB");
        let lines: Vec<&str> = sdp.split('\n').collect();
        assert_eq!(lines[..5], ["a=extmap-allow-mixed", "a=ice-ufrag:abcd", &format!("a=ice-pwd:{}", "p".repeat(24)), "a=ice-options:trickle", "a=fingerprint:sha-256 AA:BB"]);
        assert!(lines.contains(&"a=rtpmap:111 opus/48000/2"));
        assert_eq!(lines.last(), Some(&"a=rtpmap:97 rtx/90000"));
        // The web client's filter keeps each line once.
        let unique: std::collections::HashSet<_> = lines.iter().collect();
        assert_eq!(unique.len(), lines.len());
    }

    #[test]
    fn answer_is_read_from_the_server_sdp() {
        let sdp = "m=audio 50001 ICE/SDP\na=fingerprint:sha-256 4A:79\nc=IN IP4 203.0.113.9\na=rtcp:50001\na=ice-ufrag:PkLE\na=ice-pwd:secret\na=fingerprint:sha-256 4A:79\na=candidate:1 1 UDP 4261412862 203.0.113.9 50001 typ host\n";
        assert_eq!(
            answer(sdp),
            Some(Answer {
                ufrag: "PkLE".into(),
                password: "secret".into(),
                fingerprint: "sha-256 4A:79".into(),
                address: "203.0.113.9:50001".parse().unwrap(),
            })
        );
        assert_eq!(answer(&sdp.replace("a=candidate", "a=other")), None);
    }

    #[test]
    fn binary_frames() {
        assert_eq!(key_package(&[9, 9]), vec![26, 9, 9]);
        assert_eq!(commit_welcome(&[1], Some(&[2])), vec![28, 1, 2]);
        assert_eq!(binary(&[0, 5, 25, 1, 2]), Some((5, 25, &[1u8, 2][..])));
        assert_eq!(binary(&[0, 5]), None);
    }
}
