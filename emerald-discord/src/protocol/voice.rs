//! What the UI and the voice engine tell each other about a call.

use crate::model::Snowflake;

/// Where to connect for a call, put together by the UI from our own
/// VOICE_STATE_UPDATE (the session) and VOICE_SERVER_UPDATE (the rest).
pub struct VoiceCall {
    /// `None` for calls in DMs.
    pub guild: Option<Snowflake>,
    pub channel: Snowflake,
    pub user: Snowflake,
    pub session_id: String,
    /// Grants this one call; not the account token.
    pub token: String,
    pub endpoint: String,
}

/// How the user wants to sound and hear, changed at any time.
#[derive(Clone, Debug, PartialEq)]
pub struct VoiceControls {
    pub muted: bool,
    pub deafened: bool,
    /// Device ids from `emerald_audio::devices`; `None` for the default.
    pub input: Option<String>,
    pub output: Option<String>,
    /// Percent, up to 200.
    pub input_volume: u16,
    pub output_volume: u16,
    pub noise_suppression: bool,
    /// Sound quieter than this, in decibels, is not sent.
    pub sensitivity: i16,
}

impl Default for VoiceControls {
    fn default() -> Self {
        Self {
            muted: false,
            deafened: false,
            input: None,
            output: None,
            input_volume: 100,
            output_volume: 100,
            noise_suppression: true,
            sensitivity: -50,
        }
    }
}

/// How far along a call is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CallStatus {
    Connecting,
    /// Connected; agreeing on end-to-end keys with the others.
    Securing,
    /// Talking. `privacy_code` is set when the call is end-to-end encrypted.
    Connected { privacy_code: Option<String> },
}

/// News from the call in `channel`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VoiceEvent {
    Status { channel: Snowflake, status: CallStatus },
    /// Who is speaking now, us included.
    Speaking { channel: Snowflake, users: Vec<Snowflake> },
    /// No microphone could be opened; the call only listens.
    MicrophoneUnavailable { channel: Snowflake },
    /// The call is over: `error` says why when it was not by choice. In
    /// English, for the log; the UI words its own message.
    Ended { channel: Snowflake, error: Option<String> },
}
