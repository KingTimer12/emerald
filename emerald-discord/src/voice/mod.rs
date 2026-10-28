//! The sound of a call: the voice gateway, WebRTC to the voice server as
//! the web client does it ([`transport`]), end-to-end encryption
//! ([`dave`]), Opus, and the microphone and speakers through
//! `emerald_audio`.
//!
//! Each call runs on its own thread with its own runtime, so a burst of
//! gateway or REST work never delays a 20 ms audio tick. [`start`] begins
//! one; dropping the [`Running`] it returns ends it.

mod dave;
mod jitter;
mod mixer;
mod session;
mod signal;
mod socket;
mod transport;

use tokio::sync::{oneshot, watch};

use crate::protocol::{NetEvent, VoiceCall, VoiceControls, VoiceEvent};

/// Where the call reports to the UI.
type Events = async_channel::Sender<NetEvent>;

/// A call in progress; dropping it hangs up.
pub struct Running {
    _stop: oneshot::Sender<()>,
}

/// Starts the call's sound on a thread of its own.
pub fn start(call: VoiceCall, controls: watch::Receiver<VoiceControls>, events: Events) -> Running {
    let (stop, stopped) = oneshot::channel();
    let channel = call.channel;
    let spawned = std::thread::Builder::new().name("emerald-voice".into()).spawn({
        let events = events.clone();
        move || {
            let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(runtime) => runtime,
                Err(e) => {
                    end(&events, channel, Some(format!("voice runtime: {e}")));
                    return;
                }
            };
            let reports = events.clone();
            let error = runtime.block_on(async move {
                tokio::select! {
                    result = session::run(call, controls, reports) => result.err().map(|e| format!("{e:#}")),
                    // Hung up by the UI.
                    _ = stopped => None,
                }
            });
            match &error {
                Some(error) => tracing::warn!("voice: call ended: {error}"),
                None => tracing::info!("voice: call ended"),
            }
            end(&events, channel, error);
        }
    });
    if let Err(e) = spawned {
        end(&events, channel, Some(format!("voice thread: {e}")));
    }
    Running { _stop: stop }
}

fn end(events: &Events, channel: crate::model::Snowflake, error: Option<String>) {
    events.try_send(NetEvent::Voice(VoiceEvent::Ended { channel, error })).ok();
}
