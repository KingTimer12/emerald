//! Gateway v10 client: zlib-stream transport compression ([`inflater`]),
//! jittered heartbeats with zombie detection, session resume and
//! exponential reconnect backoff. Each connection runs in [`connection`];
//! its events become `NetEvent`s in [`dispatch`].

mod connection;
mod dispatch;
mod inflater;

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::{mpsc, watch};
use tokio::time::sleep;

use crate::identity::Identity;
use crate::protocol::{Link, NetEvent, OwnPresence};
use crate::urls;

/// Raw gateway payloads from the UI (op 8, op 37), sent once a session is up.
pub type OpsRx = mpsc::UnboundedReceiver<serde_json::Value>;

/// The user's chosen presence; `None` lets Discord apply the account's saved
/// status, which is what the web client's `"unknown"` means.
pub type PresenceRx = watch::Receiver<Option<OwnPresence>>;

/// Where the gateway reports to the UI.
type Events = async_channel::Sender<NetEvent>;

/// A session Discord can resume.
struct Session {
    id: String,
    resume_url: String,
}

/// What carries over from one connection to the next.
#[derive(Default)]
struct State {
    session: Option<Session>,
    /// The last sequence number seen, for heartbeats and RESUME.
    seq: Option<u64>,
    /// READY or RESUMED arrived on the current connection.
    ready: bool,
}

/// How a connection ended, and so what the next one does.
enum Outcome {
    Resume,
    Fresh,
    Fatal(String),
    /// 4004: the token is no longer valid; reconnecting cannot help.
    AuthFailed,
}

pub async fn run(identity: Arc<Identity>, events: Events, mut presence: PresenceRx, mut ops: OpsRx) {
    let mut state = State::default();
    let mut failures: u32 = 0;

    loop {
        let url = state
            .session
            .as_ref()
            .map_or(urls::GATEWAY.as_str(), |s| s.resume_url.as_str())
            .to_owned();
        state.ready = false;
        let resuming = state.session.is_some();
        let link = if resuming { Link::Resuming } else { Link::Connecting };
        events.try_send(NetEvent::Status(link)).ok();
        tracing::info!(resume = resuming, "gateway: connecting");

        let outcome = connection::connect(&url, &identity, &mut state, &events, &mut presence, &mut ops)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!("gateway: {e:#}");
                Outcome::Resume
            });

        match outcome {
            // `close_outcome` logged why, in English; `reason` is translated.
            Outcome::Fatal(reason) => {
                events.try_send(NetEvent::Fatal(reason)).ok();
                return;
            }
            Outcome::AuthFailed => {
                tracing::warn!("gateway: token rejected (4004)");
                events.try_send(NetEvent::AuthExpired).ok();
                return;
            }
            Outcome::Fresh => {
                tracing::info!("gateway: starting a new session");
                state.session = None;
                state.seq = None;
            }
            Outcome::Resume => {}
        }

        failures = if state.ready { 0 } else { failures + 1 };
        let delay = Duration::from_secs(1u64 << failures.min(6)).min(Duration::from_secs(60))
            + jitter(Duration::from_secs(2));
        tracing::info!(attempt = failures, "gateway: reconnecting in {}s", delay.as_secs());
        events
            .try_send(NetEvent::Status(Link::Reconnecting(delay.as_secs())))
            .ok();
        sleep(delay).await;
    }
}

/// Up to `max`, varying from call to call, so clients do not reconnect in
/// lockstep.
fn jitter(max: Duration) -> Duration {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos() as u64);
    let max = max.as_millis().max(1) as u64;
    Duration::from_millis(nanos % max)
}
