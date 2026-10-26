//! One gateway connection: the handshake (IDENTIFY or RESUME), heartbeats,
//! presence changes and ops from the UI going out, and frames coming in
//! until the connection ends.

use std::time::Duration;

use anyhow::{Context as _, Result, anyhow};
use futures_util::{SinkExt, StreamExt};
use tokio::time::{Instant, sleep, timeout};
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;

use super::dispatch::dispatch;
use super::inflater::Inflater;
use super::{Events, OpsRx, Outcome, PresenceRx, State, jitter};
use crate::identity::{self, Identity};
use crate::packets::gateway::{self as packets, Hello, Payload, op};
use crate::urls;

pub(super) async fn connect(
    url: &str,
    identity: &Identity,
    state: &mut State,
    events: &Events,
    presence: &mut PresenceRx,
    ops: &mut OpsRx,
) -> Result<Outcome> {
    // Same query order and upgrade headers as the web client.
    let mut request = format!(
        "{url}/?encoding=json&v={}&compress=zlib-stream",
        urls::API_VERSION
    )
    .into_client_request()?;
    let headers = request.headers_mut();
    headers.insert("user-agent", HeaderValue::from_static(identity::USER_AGENT));
    headers.insert("origin", HeaderValue::from_static(urls::WEB.as_str()));
    headers.insert("accept-language", HeaderValue::from_str(&identity.accept_language())?);
    headers.insert("cache-control", HeaderValue::from_static("no-cache"));
    headers.insert("pragma", HeaderValue::from_static("no-cache"));
    let (ws, _) = timeout(Duration::from_secs(15), tokio_tungstenite::connect_async(request))
        .await
        .context("connect timeout")??;
    let (mut tx, mut rx) = ws.split();
    let mut inflater = Inflater::new();

    let interval = loop {
        let frame = timeout(Duration::from_secs(15), rx.next())
            .await
            .context("hello timeout")?
            .ok_or_else(|| anyhow!("closed before hello"))??;
        let WsMessage::Binary(bytes) = frame else { continue };
        let Some(text) = inflater.push(&bytes)? else { continue };
        let payload: Payload = serde_json::from_str(text)?;
        if payload.op == op::HELLO {
            let hello: Hello = serde_json::from_str(payload.d.context("hello without d")?.get())?;
            tracing::debug!(interval_ms = hello.heartbeat_interval, "gateway: hello");
            break Duration::from_millis(hello.heartbeat_interval);
        }
    };

    let token = identity.token();
    // Marks the current value seen; later changes go out as op 3.
    let chosen = presence.borrow_and_update().clone();
    let status = chosen.as_ref().map_or("unknown", |p| p.status.as_str());
    let resuming = state.session.is_some();
    let handshake = match state.session.as_ref() {
        Some(s) => packets::resume(token.expose(), &s.id, state.seq),
        None => packets::identify(token.expose(), &identity.properties, status),
    };
    tx.send(WsMessage::Text(handshake.to_string().into())).await?;
    if resuming && let Some(chosen) = &chosen {
        // RESUME carries no presence; reassert one picked while disconnected.
        tx.send(WsMessage::Text(packets::presence_update(chosen).to_string().into())).await?;
    }

    let mut heartbeat = tokio::time::interval_at(Instant::now() + jitter(interval), interval);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut acked = true;

    loop {
        tokio::select! {
            // The sender lives as long as the network thread, so `changed`
            // only errors on shutdown.
            Some(op) = ops.recv(), if state.ready => {
                tx.send(WsMessage::Text(op.to_string().into())).await?;
            }
            Ok(()) = presence.changed() => {
                let Some(chosen) = presence.borrow_and_update().clone() else { continue };
                tx.send(WsMessage::Text(packets::presence_update(&chosen).to_string().into())).await?;
            }
            _ = heartbeat.tick() => {
                if !acked {
                    // Zombied connection: no ACK since the last beat.
                    tracing::warn!("gateway: no heartbeat ACK, reconnecting");
                    return Ok(Outcome::Resume);
                }
                acked = false;
                tx.send(WsMessage::Text(packets::heartbeat(state.seq).to_string().into())).await?;
            }
            frame = rx.next() => {
                let Some(frame) = frame else {
                    tracing::info!("gateway: connection ended");
                    return Ok(Outcome::Resume);
                };
                match frame? {
                    WsMessage::Binary(bytes) => {
                        let Some(text) = inflater.push(&bytes)? else { continue };
                        let payload: Payload = serde_json::from_str(text)?;
                        if payload.s.is_some() {
                            state.seq = payload.s;
                        }
                        match payload.op {
                            op::DISPATCH => dispatch(payload.t, payload.d, identity, state, events),
                            op::HEARTBEAT => {
                                tx.send(WsMessage::Text(packets::heartbeat(state.seq).to_string().into())).await?;
                            }
                            op::RECONNECT => {
                                tracing::info!("gateway: Discord asked to reconnect");
                                return Ok(Outcome::Resume);
                            }
                            op::INVALID_SESSION => {
                                let resumable = payload.d.is_some_and(|d| d.get() == "true");
                                tracing::warn!(resumable, "gateway: invalid session");
                                sleep(Duration::from_secs(1) + jitter(Duration::from_secs(4))).await;
                                return Ok(if resumable { Outcome::Resume } else { Outcome::Fresh });
                            }
                            op::HEARTBEAT_ACK => acked = true,
                            _ => {}
                        }
                    }
                    WsMessage::Close(frame) => {
                        let code = frame.map(|f| u16::from(f.code));
                        tracing::info!(?code, "gateway: closed by Discord");
                        return Ok(close_outcome(code));
                    }
                    _ => {}
                }
            }
        }
    }
}

/// What a close code means for the next connection.
fn close_outcome(code: Option<u16>) -> Outcome {
    match code {
        Some(4004) => Outcome::AuthFailed,
        Some(code @ 4010..=4014) => {
            tracing::error!(code, "gateway: Discord refused the session for good");
            Outcome::Fatal(t!("connection.refused", code = code))
        }
        Some(4007 | 4009) => Outcome::Fresh,
        _ => Outcome::Resume,
    }
}
