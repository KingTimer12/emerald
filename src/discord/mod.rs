mod build;
pub mod cache;
mod commands;
pub mod gateway;
pub mod identity;
pub mod model;
pub mod permissions;
mod protocol;
pub mod rest;
pub mod token;

use std::sync::Arc;

use model::Status;
pub use protocol::{Action, Command, IndexTarget, Link, NetEvent, NewPoll, Page, ReactionEvent, ReactionSource, ReplyTo, Snapshot, ThreadPage, Upload};
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
                let accept_language = identity::accept_language(&identity::locale());
                let build_number = build::current(&accept_language).await;
                tracing::info!("client build {build_number}");
                let identity = Arc::new(identity::Identity::new(token, build_number));
                let cache = tokio::task::spawn_blocking(|| {
                    let cache = cache::Cache::open_default()?;
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
                        Command::QueryMembers { guild, query } => {
                            ops_tx.send(gateway::query_members(guild, &query)).ok();
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
