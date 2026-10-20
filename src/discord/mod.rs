pub mod cache;
mod commands;
pub mod gateway;
pub mod identity;
pub mod model;
pub mod permissions;
mod protocol;
pub mod rest;

use std::sync::Arc;

use model::Status;
pub use protocol::{Action, Command, Link, NetEvent, Snapshot};
use tokio::sync::{mpsc, watch};

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
                tokio::spawn(gateway::run(identity.clone(), event_tx.clone(), presence_rx, ops_rx));
                if let Some(dir) = cache::dir() {
                    tokio::task::spawn_blocking(move || cache::prune(&dir));
                }
                while let Some(cmd) = cmd_rx.recv().await {
                    match cmd {
                        Command::SetStatus(status) => {
                            presence_tx.send_replace(Some(status));
                        }
                        Command::SetLocale(locale) => identity.set_locale(&locale),
                        Command::RequestMembers { guild, users } => {
                            ops_tx.send(gateway::request_members(guild, &users)).ok();
                        }
                        Command::SearchMembers { guild, query, nonce } => {
                            ops_tx.send(gateway::search_members(guild, &query, &nonce)).ok();
                        }
                        Command::SubscribeGuild { guild, members } => {
                            let members = members.as_ref().map(|(c, r)| (*c, r.as_slice()));
                            ops_tx.send(gateway::subscribe_guild(guild, members)).ok();
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
