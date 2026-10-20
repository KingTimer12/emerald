//! Gateway v10 client: zlib-stream transport compression, jittered
//! heartbeats with zombie detection, session resume and exponential
//! reconnect backoff. Unhandled dispatch types are dropped without parsing.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context as _, Result, anyhow};
use flate2::{Decompress, FlushDecompress};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::json;
use serde_json::value::RawValue;
use tokio::sync::{mpsc, watch};
use tokio::time::{Instant, sleep, timeout};
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;

use super::{Link, NetEvent, Snapshot};
use super::identity::{self, Identity};
use super::model::{
    Channel, Guild, MemberListUpdate, MemberUpdate, MembersChunk, Message, MessageAck, MessageDelete, MessageUpdate,
    Presence, Ready, ReadySupplemental, Relationship, RelationshipRemove, RoleDelete, RoleUpdate, Snowflake, Status,
    TypingStart, User,
};

/// Raw gateway payloads from the UI (op 8, op 37), sent once a session is up.
pub type OpsRx = mpsc::UnboundedReceiver<serde_json::Value>;

/// The user's chosen presence; `None` lets Discord apply the account's saved
/// status, which is what the web client's `"unknown"` means.
pub type PresenceRx = watch::Receiver<Option<Status>>;

const GATEWAY: &str = "wss://gateway.discord.gg";
const ZLIB_SUFFIX: [u8; 4] = [0x00, 0x00, 0xff, 0xff];
/// Decompression buffers above this size (e.g. after READY) are released.
const BUFFER_TRIM: usize = 256 * 1024;

struct Session {
    id: String,
    resume_url: String,
}

enum Outcome {
    Resume,
    Fresh,
    Fatal(String),
    /// 4004: the token is no longer valid; reconnecting cannot help.
    AuthFailed,
}

#[derive(Deserialize)]
struct Payload<'a> {
    op: u8,
    #[serde(borrow, default)]
    d: Option<&'a RawValue>,
    #[serde(default)]
    s: Option<u64>,
    #[serde(borrow, default)]
    t: Option<&'a str>,
}

#[derive(Deserialize)]
struct Hello {
    heartbeat_interval: u64,
}

pub async fn run(
    identity: Arc<Identity>,
    events: async_channel::Sender<NetEvent>,
    mut presence: PresenceRx,
    mut ops: OpsRx,
) {
    let mut session: Option<Session> = None;
    let mut seq: Option<u64> = None;
    let mut failures: u32 = 0;

    loop {
        let url = session
            .as_ref()
            .map_or(GATEWAY, |s| s.resume_url.as_str())
            .to_owned();
        let mut ready = false;
        let link = if session.is_some() { Link::Resuming } else { Link::Connecting };
        events.try_send(NetEvent::Status(link)).ok();

        let outcome = connect(
            &url,
            &identity,
            &mut session,
            &mut seq,
            &events,
            &mut ready,
            &mut presence,
            &mut ops,
        )
        .await
            .unwrap_or_else(|e| {
                log::warn!("gateway: {e:#}");
                Outcome::Resume
            });

        match outcome {
            Outcome::Fatal(reason) => {
                log::error!("gateway: {reason}");
                events.try_send(NetEvent::Fatal(reason)).ok();
                return;
            }
            Outcome::AuthFailed => {
                log::warn!("gateway: token rejected (4004)");
                events.try_send(NetEvent::AuthExpired).ok();
                return;
            }
            Outcome::Fresh => {
                session = None;
                seq = None;
            }
            Outcome::Resume => {}
        }

        failures = if ready { 0 } else { failures + 1 };
        let delay = Duration::from_secs(1u64 << failures.min(6)).min(Duration::from_secs(60))
            + jitter(Duration::from_secs(2));
        events
            .try_send(NetEvent::Status(Link::Reconnecting(delay.as_secs())))
            .ok();
        sleep(delay).await;
    }
}

async fn connect(
    url: &str,
    identity: &Identity,
    session: &mut Option<Session>,
    seq: &mut Option<u64>,
    events: &async_channel::Sender<NetEvent>,
    ready: &mut bool,
    presence: &mut PresenceRx,
    ops: &mut OpsRx,
) -> Result<Outcome> {
    // Same query order and upgrade headers as the web client.
    let mut request = format!(
        "{url}/?encoding=json&v={}&compress=zlib-stream",
        identity::API_VERSION
    )
    .into_client_request()?;
    let headers = request.headers_mut();
    headers.insert("user-agent", HeaderValue::from_static(identity::USER_AGENT));
    headers.insert("origin", HeaderValue::from_static("https://discord.com"));
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
        if payload.op == 10 {
            let hello: Hello = serde_json::from_str(payload.d.context("hello without d")?.get())?;
            break Duration::from_millis(hello.heartbeat_interval);
        }
    };

    let token = identity.token();
    // Marks the current value seen; later changes go out as op 3.
    let chosen = *presence.borrow_and_update();
    let status = chosen.map_or("unknown", Status::as_str);
    let resuming = session.is_some();
    let handshake = match session.as_ref() {
        Some(s) => json!({
            "op": 6,
            "d": { "token": token.expose(), "session_id": s.id, "seq": *seq },
        }),
        // Field order follows the web client's IDENTIFY; qos_token is omitted.
        None => json!({
            "op": 2,
            "d": {
                "token": token.expose(),
                "capabilities": identity::CAPABILITIES,
                "properties": identity.properties,
                "presence": { "status": status, "since": 0, "activities": [], "afk": false },
                "compress": false,
                "client_state": { "guild_versions": {} },
            },
        }),
    };
    tx.send(WsMessage::Text(handshake.to_string().into())).await?;
    if resuming && chosen.is_some() {
        // RESUME carries no presence; reassert one picked while disconnected.
        tx.send(WsMessage::Text(presence_update(status).to_string().into())).await?;
    }

    let mut heartbeat = tokio::time::interval_at(Instant::now() + jitter(interval), interval);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut acked = true;

    loop {
        tokio::select! {
            // The sender lives as long as the network thread, so `changed`
            // only errors on shutdown.
            Some(op) = ops.recv(), if *ready => {
                tx.send(WsMessage::Text(op.to_string().into())).await?;
            }
            Ok(()) = presence.changed() => {
                let Some(status) = *presence.borrow_and_update() else { continue };
                tx.send(WsMessage::Text(presence_update(status.as_str()).to_string().into())).await?;
            }
            _ = heartbeat.tick() => {
                if !acked {
                    // Zombied connection: no ACK since the last beat.
                    return Ok(Outcome::Resume);
                }
                acked = false;
                tx.send(WsMessage::Text(json!({ "op": 1, "d": *seq }).to_string().into())).await?;
            }
            frame = rx.next() => {
                let Some(frame) = frame else { return Ok(Outcome::Resume) };
                match frame? {
                    WsMessage::Binary(bytes) => {
                        let Some(text) = inflater.push(&bytes)? else { continue };
                        let payload: Payload = serde_json::from_str(text)?;
                        if payload.s.is_some() {
                            *seq = payload.s;
                        }
                        match payload.op {
                            0 => dispatch(payload.t, payload.d, identity, session, events, ready),
                            1 => {
                                tx.send(WsMessage::Text(json!({ "op": 1, "d": *seq }).to_string().into())).await?;
                            }
                            7 => return Ok(Outcome::Resume),
                            9 => {
                                let resumable = payload.d.is_some_and(|d| d.get() == "true");
                                sleep(Duration::from_secs(1) + jitter(Duration::from_secs(4))).await;
                                return Ok(if resumable { Outcome::Resume } else { Outcome::Fresh });
                            }
                            11 => acked = true,
                            _ => {}
                        }
                    }
                    WsMessage::Close(frame) => {
                        return Ok(close_outcome(frame.map(|f| u16::from(f.code))));
                    }
                    _ => {}
                }
            }
        }
    }
}

fn close_outcome(code: Option<u16>) -> Outcome {
    match code {
        Some(4004) => Outcome::AuthFailed,
        Some(code @ 4010..=4014) => Outcome::Fatal(t!("connection.refused", code = code)),
        Some(4007 | 4009) => Outcome::Fresh,
        _ => Outcome::Resume,
    }
}

fn dispatch(
    kind: Option<&str>,
    data: Option<&RawValue>,
    identity: &Identity,
    session: &mut Option<Session>,
    events: &async_channel::Sender<NetEvent>,
    ready: &mut bool,
) {
    let (Some(kind), Some(data)) = (kind, data) else { return };
    let data = data.get();
    let event = match kind {
        "READY" => {
            let Some(r) = parse::<Ready>(kind, data) else { return };
            if let Some(token) = r.auth_token {
                identity.replace_token(token);
            }
            log::info!(
                "READY: {} guilds, {} private channels, {} users, {} relationships",
                r.guilds.len(),
                r.private_channels.len(),
                r.users.len(),
                r.relationships.len(),
            );
            identity.set_session_id(r.session_id.clone());
            *session = Some(Session {
                id: r.session_id,
                resume_url: r.resume_gateway_url,
            });
            *ready = true;
            let members = r
                .guilds
                .iter()
                .zip(r.merged_members)
                .filter_map(|(guild, mut members)| Some((guild.id, members.pop()?)))
                .collect();
            let read_states = r
                .read_state
                .entries
                .into_iter()
                .filter(|state| state.read_state_type == 0)
                .filter_map(|state| Some((state.id, state.last_message_id?, state.mention_count)))
                .collect();
            NetEvent::Ready(Box::new(Snapshot {
                me: r.user,
                guilds: r.guilds,
                dms: r.private_channels,
                users: r.users,
                relationships: r.relationships,
                members,
                read_states,
            }))
        }
        "READY_SUPPLEMENTAL" => match parse::<ReadySupplemental>(kind, data) {
            Some(r) => NetEvent::Presences(friend_presences(r.merged_presences.friends)),
            None => return,
        },
        "PRESENCE_UPDATE" => match parse::<Presence>(kind, data) {
            Some(p) if p.guild_id.is_none() => NetEvent::Presences(friend_presences(vec![p])),
            _ => return,
        },
        "RELATIONSHIP_ADD" | "RELATIONSHIP_UPDATE" => match parse::<Relationship>(kind, data) {
            Some(r) => NetEvent::RelationshipAdd(r),
            None => return,
        },
        "RELATIONSHIP_REMOVE" => match parse::<RelationshipRemove>(kind, data) {
            Some(r) => NetEvent::RelationshipRemove(r.id),
            None => return,
        },
        "USER_UPDATE" => match parse::<User>(kind, data) {
            Some(u) => NetEvent::UserUpdate(u),
            None => return,
        },
        "RESUMED" => {
            *ready = true;
            NetEvent::Status(Link::Connected)
        }
        "GUILD_CREATE" => match parse::<Guild>(kind, data) {
            Some(g) => NetEvent::GuildCreate(g),
            None => return,
        },
        "GUILD_DELETE" => match parse::<Guild>(kind, data) {
            Some(g) if !g.unavailable => NetEvent::GuildDelete(g.id),
            _ => return,
        },
        // An update replaces the channel, which a create also does.
        "CHANNEL_CREATE" | "CHANNEL_UPDATE" => match parse::<Channel>(kind, data) {
            Some(c) => NetEvent::ChannelCreate(c),
            None => return,
        },
        "CHANNEL_DELETE" => match parse::<Channel>(kind, data) {
            Some(c) => NetEvent::ChannelDelete(c),
            None => return,
        },
        "MESSAGE_CREATE" => match parse::<Message>(kind, data) {
            Some(m) => NetEvent::MessageCreate(m),
            None => return,
        },
        "MESSAGE_UPDATE" => match parse::<MessageUpdate>(kind, data) {
            Some(m) => NetEvent::MessageUpdate(m),
            None => return,
        },
        "MESSAGE_DELETE" => match parse::<MessageDelete>(kind, data) {
            Some(m) => NetEvent::MessageDelete(m),
            None => return,
        },
        "GUILD_MEMBERS_CHUNK" => match parse::<MembersChunk>(kind, data) {
            Some(chunk) => NetEvent::MembersChunk(chunk),
            None => return,
        },
        "GUILD_MEMBER_LIST_UPDATE" => match parse::<MemberListUpdate>(kind, data) {
            Some(update) => NetEvent::MemberList(update),
            None => return,
        },
        "GUILD_MEMBER_UPDATE" => match parse::<MemberUpdate>(kind, data) {
            Some(update) => NetEvent::MemberUpdate(update),
            None => return,
        },
        "GUILD_ROLE_CREATE" | "GUILD_ROLE_UPDATE" => match parse::<RoleUpdate>(kind, data) {
            Some(update) => NetEvent::RoleUpdate(update),
            None => return,
        },
        "GUILD_ROLE_DELETE" => match parse::<RoleDelete>(kind, data) {
            Some(delete) => NetEvent::RoleDelete(delete),
            None => return,
        },
        "MESSAGE_ACK" => match parse::<MessageAck>(kind, data) {
            Some(ack) => NetEvent::MessageAck(ack),
            None => return,
        },
        "TYPING_START" => match parse::<TypingStart>(kind, data) {
            Some(t) => NetEvent::TypingStart(t),
            None => return,
        },
        _ => return,
    };
    events.try_send(event).ok();
}

/// Op 8 for specific users, the shape the web client uses for message authors.
pub fn request_members(guild: Snowflake, users: &[Snowflake]) -> serde_json::Value {
    let users: Vec<String> = users.iter().map(Snowflake::to_string).collect();
    json!({
        "op": 8,
        "d": { "guild_id": [guild.to_string()], "user_ids": users, "presences": false },
    })
}

/// Op 8 with a name prefix, as the member search box sends it.
pub fn search_members(guild: Snowflake, query: &str, nonce: &str) -> serde_json::Value {
    json!({
        "op": 8,
        "d": { "guild_id": [guild.to_string()], "query": query, "limit": 100, "presences": false, "nonce": nonce },
    })
}

/// Op 37, as the web client sends on opening a guild: typing, activity and
/// thread events, plus the member list rows of `channel` in `ranges`.
pub fn subscribe_guild(guild: Snowflake, channel: Option<(Snowflake, &[[usize; 2]])>) -> serde_json::Value {
    let mut subscription = json!({ "typing": true, "activities": true, "threads": true });
    if let Some((channel, ranges)) = channel {
        subscription["channels"] = json!({ channel.to_string(): ranges });
    }
    json!({
        "op": 37,
        "d": { "subscriptions": { guild.to_string(): subscription } },
    })
}

fn presence_update(status: &str) -> serde_json::Value {
    json!({ "op": 3, "d": { "status": status, "since": 0, "activities": [], "afk": false } })
}

fn friend_presences(presences: Vec<Presence>) -> Vec<(super::model::Snowflake, String)> {
    presences
        .into_iter()
        .filter_map(|p| Some((p.user()?, p.status)))
        .collect()
}

fn parse<T: DeserializeOwned>(kind: &str, data: &str) -> Option<T> {
    serde_json::from_str(data)
        .map_err(|e| log::warn!("skipping malformed {kind}: {e}"))
        .ok()
}

fn jitter(max: Duration) -> Duration {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos() as u64);
    let max = max.as_millis().max(1) as u64;
    Duration::from_millis(nanos % max)
}

/// One zlib context per connection; frames are complete when they end in
/// the Z_SYNC_FLUSH suffix.
struct Inflater {
    zlib: Decompress,
    input: Vec<u8>,
    output: Vec<u8>,
}

impl Inflater {
    fn new() -> Self {
        Self {
            zlib: Decompress::new(true),
            input: Vec::new(),
            output: Vec::new(),
        }
    }

    fn push(&mut self, data: &[u8]) -> Result<Option<&str>> {
        if self.input.is_empty() && self.output.capacity() > BUFFER_TRIM {
            self.output = Vec::new();
        }
        self.input.extend_from_slice(data);
        if !self.input.ends_with(&ZLIB_SUFFIX) {
            return Ok(None);
        }

        self.output.clear();
        let mut consumed = 0;
        loop {
            if self.output.len() == self.output.capacity() {
                self.output.reserve((self.input.len() * 4).max(8 * 1024));
            }
            let before = self.zlib.total_in();
            self.zlib
                .decompress_vec(&self.input[consumed..], &mut self.output, FlushDecompress::Sync)?;
            consumed += (self.zlib.total_in() - before) as usize;
            if consumed >= self.input.len() && self.output.len() < self.output.capacity() {
                break;
            }
        }

        if self.input.capacity() > BUFFER_TRIM {
            self.input = Vec::new();
        } else {
            self.input.clear();
        }
        Ok(Some(std::str::from_utf8(&self.output)?))
    }
}
