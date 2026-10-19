pub mod cache;
pub mod gateway;
pub mod identity;
pub mod model;
pub mod permissions;
pub mod rest;

use std::sync::Arc;

use model::{
    Channel, Guild, Member, MembersChunk, Message, MessageAck, MessageDelete, MessageUpdate,
    Profile, Relationship, Snowflake, Status, TypingStart, User,
};
use tokio::sync::{mpsc, watch};

/// A user-initiated REST action whose outcome the UI reports back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    FriendRequest,
    Relationship,
    JoinGuild,
    CreateGuild,
    UpdateProfile,
}

/// Events flowing from the network thread to the UI.
pub enum NetEvent {
    Status(String),
    Fatal(String),
    /// The token was rejected (gateway 4004 / REST 401); sign in again.
    AuthExpired,
    /// Answer to `Command::LogOut`; `error` is set if the server-side
    /// revocation failed (the local sign-out still happens).
    LoggedOut {
        error: Option<String>,
    },
    Ready {
        me: User,
        guilds: Vec<Guild>,
        dms: Vec<Channel>,
        users: Vec<User>,
        relationships: Vec<Relationship>,
        /// The current user's membership per guild.
        members: Vec<(Snowflake, Member)>,
        /// Last read message per channel.
        read_states: Vec<(Snowflake, Snowflake)>,
    },
    MembersChunk(MembersChunk),
    /// A channel was read up to `message_id`, here or on another device.
    MessageAck(MessageAck),
    /// Friend presences as `(user, status)`.
    Presences(Vec<(Snowflake, String)>),
    RelationshipAdd(Relationship),
    RelationshipRemove(Snowflake),
    UserUpdate(User),
    /// A DM opened with `Command::OpenDm`; selected when `select` is set.
    DmOpened {
        channel: Channel,
        select: bool,
    },
    /// Image bytes (avatar or sticker) for `url`; `None` if the download failed.
    Avatar {
        url: String,
        bytes: Option<Vec<u8>>,
    },
    ActionDone(Action),
    ActionFailed {
        action: Action,
        error: String,
    },
    GuildCreate(Guild),
    GuildDelete(Snowflake),
    ChannelCreate(Channel),
    ChannelDelete(Channel),
    MessageCreate(Message),
    MessageUpdate(MessageUpdate),
    MessageDelete(MessageDelete),
    TypingStart(TypingStart),
    Profile(Profile),
    History {
        channel: Snowflake,
        before: Option<Snowflake>,
        messages: Vec<Message>,
    },
    HistoryFailed {
        channel: Snowflake,
        forbidden: bool,
        error: String,
    },
    /// The message with `nonce` was not sent; `content` lets the UI give
    /// the text back to the user.
    SendFailed {
        channel: Snowflake,
        nonce: String,
        content: String,
        error: String,
    },
}

/// Requests flowing from the UI to the network thread.
pub enum Command {
    History {
        channel: Snowflake,
        before: Option<Snowflake>,
    },
    Send {
        channel: Snowflake,
        content: String,
        nonce: String,
    },
    SendFriendRequest {
        username: String,
    },
    AcceptFriend {
        user: Snowflake,
    },
    /// Removes a friend, declines an incoming request or cancels an outgoing one.
    RemoveRelationship {
        user: Snowflake,
    },
    /// Opens (or reopens) the DM with `user`. Unselected opens run in the
    /// background, so they never show a captcha and fail silently.
    OpenDm {
        user: Snowflake,
        select: bool,
    },
    JoinGuild {
        code: String,
    },
    CreateGuild {
        name: String,
    },
    SetGlobalName {
        name: Option<String>,
    },
    SetStatus(Status),
    /// Gateway op 8: fetch these members' nicknames and roles.
    RequestMembers {
        guild: Snowflake,
        users: Vec<Snowflake>,
    },
    /// Gateway op 37: what the web client sends on opening a guild, so the
    /// gateway delivers its typing and thread events.
    SubscribeGuild {
        guild: Snowflake,
    },
    /// Marks `channel` read up to `message`.
    Ack {
        channel: Snowflake,
        message: Snowflake,
    },
    Typing {
        channel: Snowflake,
    },
    FetchProfile {
        user: Snowflake,
    },
    /// Downloads an avatar or sticker image.
    FetchAvatar {
        url: String,
    },
    LogOut,
}

/// Runs all networking on one dedicated OS thread with a single-threaded
/// tokio runtime, keeping the UI thread free and the thread count minimal.
pub fn spawn(
    token: String,
    status: Option<Status>,
) -> (
    async_channel::Receiver<NetEvent>,
    mpsc::UnboundedSender<Command>,
) {
    let (event_tx, event_rx) = async_channel::unbounded();
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<Command>();
    let identity = Arc::new(identity::Identity::new(token));
    let (presence_tx, presence_rx) = watch::channel(status);
    let (ops_tx, ops_rx) = mpsc::unbounded_channel::<serde_json::Value>();

    std::thread::Builder::new()
        .name("emerald-net".into())
        .spawn(move || {
            rustls::crypto::ring::default_provider()
                .install_default()
                .ok();
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime");
            runtime.block_on(async move {
                let rest = match rest::Rest::new(identity.clone()) {
                    Ok(rest) => Arc::new(rest),
                    Err(e) => {
                        event_tx.try_send(NetEvent::Fatal(e.to_string())).ok();
                        return;
                    }
                };
                tokio::spawn(gateway::run(identity, event_tx.clone(), presence_rx, ops_rx));
                if let Some(dir) = cache::dir() {
                    tokio::task::spawn_blocking(move || cache::prune(&dir));
                }
                while let Some(cmd) = cmd_rx.recv().await {
                    match cmd {
                        Command::SetStatus(status) => {
                            presence_tx.send_replace(Some(status));
                        }
                        Command::RequestMembers { guild, users } => {
                            ops_tx.send(gateway::request_members(guild, &users)).ok();
                        }
                        Command::SubscribeGuild { guild } => {
                            ops_tx.send(gateway::subscribe_guild(guild)).ok();
                        }
                        cmd => {
                            tokio::spawn(handle(rest.clone(), cmd, event_tx.clone()));
                        }
                    }
                }
            });
        })
        .expect("spawn network thread");

    (event_rx, cmd_tx)
}

async fn handle(rest: Arc<rest::Rest>, cmd: Command, events: async_channel::Sender<NetEvent>) {
    let event = match cmd {
        Command::History { channel, before } => match rest.messages(channel, before).await {
            Ok(messages) => NetEvent::History {
                channel,
                before,
                messages,
            },
            Err(e) if e.is::<rest::Unauthorized>() => NetEvent::AuthExpired,
            Err(e) => NetEvent::HistoryFailed {
                channel,
                forbidden: e.is::<rest::Forbidden>(),
                error: e.to_string(),
            },
        },
        Command::Send {
            channel,
            content,
            nonce,
        } => match with_captcha!(|captcha| rest.send_message(channel, &content, &nonce, captcha).await) {
            // Usually the gateway echo arrives first; the UI drops the duplicate.
            Ok(message) => NetEvent::MessageCreate(message),
            Err(e) if e.is::<rest::Unauthorized>() => NetEvent::AuthExpired,
            Err(e) => NetEvent::SendFailed {
                channel,
                nonce,
                content,
                error: e.to_string(),
            },
        },
        Command::SendFriendRequest { username } => outcome(
            Action::FriendRequest,
            with_captcha!(|captcha| rest.send_friend_request(&username, captcha).await),
        ),
        Command::AcceptFriend { user } => {
            outcome(Action::Relationship, rest.accept_friend(user).await)
        }
        Command::RemoveRelationship { user } => {
            outcome(Action::Relationship, rest.remove_relationship(user).await)
        }
        Command::OpenDm { user, select: true } => {
            match with_captcha!(|captcha| rest.open_dm(user, captcha).await) {
                Ok(channel) => NetEvent::DmOpened {
                    channel,
                    select: true,
                },
                Err(e) => failure(Action::Relationship, e),
            }
        }
        Command::OpenDm { user, select: false } => match rest.open_dm(user, None).await {
            Ok(channel) => NetEvent::DmOpened {
                channel,
                select: false,
            },
            Err(e) => {
                log::warn!("background DM with {user}: {e:#}");
                return;
            }
        },
        Command::JoinGuild { code } => outcome(
            Action::JoinGuild,
            with_captcha!(|captcha| rest.join_guild(&code, captcha).await),
        ),
        Command::CreateGuild { name } => outcome(
            Action::CreateGuild,
            with_captcha!(|captcha| rest.create_guild(&name, captcha).await),
        ),
        Command::SetGlobalName { name } => match rest.set_global_name(name.as_deref()).await {
            Ok(user) => {
                events.try_send(NetEvent::UserUpdate(user)).ok();
                NetEvent::ActionDone(Action::UpdateProfile)
            }
            Err(e) => failure(Action::UpdateProfile, e),
        },
        Command::LogOut => NetEvent::LoggedOut {
            // A token the server already rejects needs no revocation.
            error: match rest.logout().await {
                Err(e) if !e.is::<rest::Unauthorized>() => Some(e.to_string()),
                _ => None,
            },
        },
        Command::Typing { channel } => {
            if let Err(e) = rest.typing(channel).await {
                log::debug!("typing in {channel}: {e:#}");
            }
            return;
        }
        Command::FetchProfile { user } => match rest.profile(user).await {
            Ok(profile) => NetEvent::Profile(profile),
            Err(e) => {
                log::warn!("profile of {user}: {e:#}");
                return;
            }
        },
        Command::FetchAvatar { url } => {
            let bytes = rest
                .cdn_image(&url)
                .await
                .map_err(|e| log::warn!("avatar {url}: {e:#}"))
                .ok();
            NetEvent::Avatar { url, bytes }
        }
        Command::Ack { channel, message } => {
            if let Err(e) = rest.ack(channel, message).await {
                log::debug!("ack {channel}/{message}: {e:#}");
            }
            return;
        }
        // Routed to the gateway before spawning.
        Command::SetStatus(_) | Command::RequestMembers { .. } | Command::SubscribeGuild { .. } => {
            return;
        }
    };
    events.try_send(event).ok();
}

/// Evaluates `$call` with `$captcha = None`; if Discord answers with a
/// captcha, shows it to the user and evaluates `$call` once more with their
/// solution. Closing the captcha window keeps the original error. A macro
/// rather than an async closure so the spawned future stays `Send`.
macro_rules! with_captcha {
    (|$captcha:ident| $call:expr) => {{
        let $captcha: Option<&rest::Solution> = None;
        match $call {
            Err(error) => match solve_captcha(&error).await {
                Some(solution) => {
                    let $captcha = Some(&solution);
                    $call
                }
                None => Err(error),
            },
            ok => ok,
        }
    }};
}
use with_captcha;

/// When `error` is a captcha challenge, lets the user solve it in a webview
/// child process and returns the solution.
async fn solve_captcha(error: &anyhow::Error) -> Option<rest::Solution> {
    let rest::CaptchaRequired(challenge) = error.downcast_ref::<rest::CaptchaRequired>()?;
    let challenge = challenge.clone();
    let input = serde_json::to_vec(&challenge).ok()?;
    // The child blocks until its window closes; keep it off the runtime thread.
    let key = tokio::task::spawn_blocking(move || crate::webview::run_child("--captcha", &input))
        .await
        .ok()
        .flatten()?;
    Some(rest::Solution::new(key, &challenge))
}

fn outcome(action: Action, result: anyhow::Result<()>) -> NetEvent {
    match result {
        Ok(()) => NetEvent::ActionDone(action),
        Err(e) => failure(action, e),
    }
}

fn failure(action: Action, error: anyhow::Error) -> NetEvent {
    if error.is::<rest::Unauthorized>() {
        return NetEvent::AuthExpired;
    }
    NetEvent::ActionFailed {
        action,
        error: error.to_string(),
    }
}
