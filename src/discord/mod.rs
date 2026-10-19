pub mod gateway;
pub mod identity;
pub mod model;
pub mod rest;

use std::sync::Arc;

use model::{
    Channel, Guild, Message, MessageDelete, MessageUpdate, Relationship, Snowflake, Status, User,
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
    },
    /// Friend presences as `(user, status)`.
    Presences(Vec<(Snowflake, String)>),
    RelationshipAdd(Relationship),
    RelationshipRemove(Snowflake),
    UserUpdate(User),
    /// A DM opened from the friends list; the UI selects it.
    DmOpened(Channel),
    /// Avatar bytes for `url`; `None` if the download failed.
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
    SendFailed {
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
    OpenDm {
        user: Snowflake,
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
                tokio::spawn(gateway::run(identity, event_tx.clone(), presence_rx));
                while let Some(cmd) = cmd_rx.recv().await {
                    match cmd {
                        Command::SetStatus(status) => {
                            presence_tx.send_replace(Some(status));
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
        Command::Send { channel, content } => match rest.send_message(channel, &content).await {
            Ok(()) => return,
            Err(e) if e.is::<rest::Unauthorized>() => NetEvent::AuthExpired,
            Err(e) => NetEvent::SendFailed {
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
        Command::OpenDm { user } => match with_captcha!(|captcha| rest.open_dm(user, captcha).await) {
            Ok(channel) => NetEvent::DmOpened(channel),
            Err(e) => failure(Action::Relationship, e),
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
        Command::FetchAvatar { url } => {
            let bytes = rest
                .cdn_image(&url)
                .await
                .map_err(|e| log::warn!("avatar {url}: {e:#}"))
                .ok();
            NetEvent::Avatar { url, bytes }
        }
        // Routed to the gateway's presence channel before spawning.
        Command::SetStatus(_) => return,
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
