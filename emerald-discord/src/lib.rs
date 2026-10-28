//! Discord as its web client speaks it: the REST endpoints ([`rest`]),
//! the gateway ([`gateway`]), the payload shapes ([`model`]), the frames
//! and bodies sent on the wire ([`packets`]) and the sound of calls
//! ([`voice`]). The interface talks to
//! the network thread [`spawn`] starts only through [`Command`] and
//! [`NetEvent`].

#[macro_use]
extern crate emerald_core;

mod client_build;
mod commands;
pub mod gateway;
pub mod identity;
pub mod model;
mod packets;
pub mod permissions;
mod protocol;
pub mod rest;
pub mod token;
pub mod urls;
mod voice;

use std::sync::Arc;

use model::Status;
pub use protocol::{
    Action, Command, IndexTarget, Link, MUTE_WINDOWS, NetEvent, NewPoll, OwnPresence, Page, Picture, PinPage, ProfileEdit,
    CallStatus, ChannelEdit, ReactionEvent, ReactionSource, ReplyTo, SearchPage, SearchQuery, SearchScope, SettingsChange, SettingsEdit,
    Snapshot, TagEdit, ThreadPage, ThreadQuery, Upload, VoiceCall, VoiceControls, VoiceEvent,
};
use tokio::sync::{mpsc, watch};

/// Runs all networking on one dedicated OS thread with a single-threaded
/// tokio runtime, keeping the UI thread free and the thread count minimal.
pub fn spawn(
    token: token::Token,
    status: Option<Status>,
) -> (
    async_channel::Receiver<NetEvent>,
    mpsc::UnboundedSender<Command>,
) {
    let (event_tx, event_rx) = async_channel::unbounded();
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<Command>();
    let (presence_tx, presence_rx) = watch::channel(status.map(|status| OwnPresence { status, custom: None }));
    let (ops_tx, ops_rx) = mpsc::unbounded_channel::<serde_json::Value>();
    // Kept between calls, so the next one starts muted when this one was.
    let (controls_tx, _) = watch::channel(VoiceControls::default());

    std::thread::Builder::new()
        .name("emerald-net".into())
        .spawn(move || {
            rustls::crypto::ring::default_provider()
                .install_default()
                .ok();
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                // Disk cache reads and frame decoding: a screenful of moving
                // emoji would otherwise start a thread each (tokio allows 512).
                .max_blocking_threads(8)
                .build()
                .expect("tokio runtime");
            runtime.block_on(async move {
                let accept_language = identity::accept_language(&identity::locale());
                let build_number = client_build::current(&accept_language).await;
                tracing::info!("client build {build_number}");
                let identity = Arc::new(identity::Identity::new(token, build_number));
                let cache = tokio::task::spawn_blocking(|| {
                    let cache = emerald_http::Cache::open_default()?;
                    cache.prune();
                    Some(Arc::new(cache))
                })
                .await
                .ok()
                .flatten();
                let rest = match rest::Rest::new(identity.clone(), cache) {
                    Ok(rest) => Arc::new(rest),
                    Err(e) => {
                        event_tx.try_send(NetEvent::Fatal(e.to_string())).ok();
                        return;
                    }
                };
                tokio::spawn(gateway::run(identity.clone(), event_tx.clone(), presence_rx, ops_rx));
                // Dropping it hangs up.
                let mut _call: Option<voice::Running> = None;
                while let Some(cmd) = cmd_rx.recv().await {
                    match cmd {
                        Command::SetPresence(presence) => {
                            presence_tx.send_replace(Some(presence));
                        }
                        Command::SetLocale(locale) => identity.set_locale(&locale),
                        Command::RequestMembers { guild, users } => {
                            ops_tx.send(packets::gateway::request_members(guild, &users)).ok();
                        }
                        Command::SearchMembers { guild, query, nonce } => {
                            ops_tx.send(packets::gateway::search_members(guild, &query, &nonce)).ok();
                        }
                        Command::QueryMembers { guild, query } => {
                            ops_tx.send(packets::gateway::query_members(guild, &query)).ok();
                        }
                        Command::VoiceState { guild, channel, self_mute, self_deaf } => {
                            ops_tx.send(packets::gateway::voice_state(guild, channel, self_mute, self_deaf)).ok();
                        }
                        Command::VoiceConnect(details) => {
                            // The old call hangs up first.
                            _call = None;
                            _call = Some(voice::start(details, controls_tx.subscribe(), event_tx.clone()));
                        }
                        Command::VoiceDisconnect => _call = None,
                        Command::VoiceControls(controls) => {
                            controls_tx.send_replace(controls);
                        }
                        Command::SubscribeGuild { guild, members } => {
                            let members = members.as_ref().map(|(c, r)| (*c, r.as_slice()));
                            ops_tx.send(packets::gateway::subscribe_guild(guild, members)).ok();
                        }
                        cmd => {
                            tokio::spawn(commands::handle(rest.clone(), cmd, event_tx.clone()));
                        }
                    }
                }
            });
        })
        .expect("spawn network thread");

    (event_rx, cmd_tx)
}

/// Forgets the active account's token and starts Emerald again, which
/// then opens the next signed-in account, or the login webview when there
/// is none.
pub fn sign_out() -> ! {
    token::forget();
    restart()
}

/// Starts Emerald again on `account`, another one signed in here.
pub fn switch_account(account: model::Snowflake) -> ! {
    if !token::switch_to(account) {
        tracing::warn!("switching to an account that is not signed in");
    }
    restart()
}

/// Opens the login window for one more account and blocks until it closes.
/// The new account becomes the active one; `false` when the window was
/// closed without signing in or the token could not be kept.
pub fn add_account() -> bool {
    let Some(token) = emerald_core::process::run_child("--login", b"").and_then(token::Token::new) else {
        tracing::info!("adding an account was cancelled");
        return false;
    };
    match token::save(&token) {
        Ok(()) => {
            tracing::info!("signed in to another account");
            true
        }
        Err(e) => {
            tracing::warn!("could not save the new account's token: {e}");
            false
        }
    }
}

/// Starts Emerald afresh on the active account.
pub fn restart() -> ! {
    // Kept API answers belong to the account that is leaving.
    if let Some(cache) = emerald_http::Cache::open_default() {
        cache.clear_responses();
    }
    // `token::take_env` already took DISCORD_TOKEN out of the environment,
    // so the new process cannot pick the old token up again.
    let error = emerald_core::process::restart();
    tracing::error!("could not restart Emerald: {error}");
    std::process::exit(1);
}
