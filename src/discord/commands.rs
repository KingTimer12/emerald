//! Runs UI commands against the REST API and turns the outcome into the
//! event the UI waits for.

use std::sync::Arc;

use super::protocol::{Action, Command, NetEvent, ThreadPage};
use super::rest::{self, Outgoing};

pub(super) async fn handle(rest: Arc<rest::Rest>, cmd: Command, events: async_channel::Sender<NetEvent>) {
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
                error: rest::describe(&e),
            },
        },
        Command::ForumPosts { forum, threads } => match rest.forum_posts(forum, &threads).await {
            Ok(first) => NetEvent::ForumPosts { forum, first },
            Err(e) => {
                tracing::warn!("forum posts of {forum}: {e:#}");
                NetEvent::ForumPosts { forum, first: Vec::new() }
            }
        },
        Command::ArchivedPosts { forum, offset } => {
            let page = rest
                .archived_posts(forum, offset)
                .await
                .map_err(|e| tracing::warn!("archived posts of {forum}: {e:#}"))
                .ok()
                .map(|mut search| {
                    for thread in &mut search.threads {
                        if let Some(member) = search.members.iter().find(|m| m.id == thread.id) {
                            thread.member = Some(member.member());
                        }
                    }
                    ThreadPage {
                        threads: search.threads,
                        first: search.first_messages,
                        has_more: search.has_more,
                    }
                });
            NetEvent::ArchivedPosts { forum, offset, page }
        }
        Command::Send {
            channel,
            content,
            nonce,
            reply,
            files,
        } => 'send: {
            // Files go up once; a captcha retry only resends the message.
            let uploaded = match rest.upload_files(channel, &files).await {
                Ok(uploaded) => uploaded,
                Err(e) if e.is::<rest::Unauthorized>() => break 'send NetEvent::AuthExpired,
                Err(e) => {
                    let error = rest::describe(&e);
                    break 'send NetEvent::SendFailed { channel, nonce, content, files, error };
                }
            };
            match with_captcha!(|captcha| rest.send_message(channel, &Outgoing { content: &content, nonce: &nonce, reply, files: &uploaded, stickers: &[] }, captcha).await) {
                // Usually the gateway echo arrives first; the UI drops the duplicate.
                Ok(message) => NetEvent::MessageCreate(message),
                Err(e) if e.is::<rest::Unauthorized>() => NetEvent::AuthExpired,
                Err(e) => NetEvent::SendFailed {
                    channel,
                    nonce,
                    content,
                    files,
                    error: rest::describe(&e),
                },
            }
        }
        Command::SendSticker { channel, sticker, nonce } => {
            let stickers = [sticker];
            match with_captcha!(|captcha| rest.send_message(channel, &Outgoing { nonce: &nonce, stickers: &stickers, ..Outgoing::default() }, captcha).await) {
                Ok(message) => NetEvent::MessageCreate(message),
                Err(e) if e.is::<rest::Unauthorized>() => NetEvent::AuthExpired,
                Err(e) => NetEvent::SendFailed {
                    channel,
                    nonce,
                    content: String::new(),
                    files: Vec::new(),
                    error: rest::describe(&e),
                },
            }
        }
        Command::SendGif { channel, gif, query, nonce } => {
            // Only bookkeeping on Discord's side; the GIF goes out either way.
            if let Err(e) = rest.select_gif(&gif.id, &query).await {
                tracing::debug!("gif select: {e:#}");
            }
            match with_captcha!(|captcha| rest.send_message(channel, &Outgoing { content: &gif.url, nonce: &nonce, ..Outgoing::default() }, captcha).await) {
                Ok(message) => NetEvent::MessageCreate(message),
                Err(e) if e.is::<rest::Unauthorized>() => NetEvent::AuthExpired,
                Err(e) => NetEvent::SendFailed {
                    channel,
                    nonce,
                    content: String::new(),
                    files: Vec::new(),
                    error: rest::describe(&e),
                },
            }
        }
        Command::ListCommands(target) => 'index: {
            // Discord answers 202 while it builds the index; the web client
            // asks again 5 seconds later, three tries in all.
            for _ in 0..3 {
                match rest.command_index(target).await {
                    Ok(Some(index)) => break 'index NetEvent::CommandIndex { target, index: Some(index) },
                    Ok(None) => tokio::time::sleep(std::time::Duration::from_secs(5)).await,
                    Err(e) if e.is::<rest::Unauthorized>() => break 'index NetEvent::AuthExpired,
                    Err(e) => {
                        tracing::warn!("command index: {}", rest::describe(&e));
                        break 'index NetEvent::CommandIndex { target, index: None };
                    }
                }
            }
            NetEvent::CommandIndex { target, index: None }
        }
        Command::Interact {
            application,
            guild,
            channel,
            data,
            nonce,
        } => outcome(Action::RunCommand, rest.run_command(application, guild, channel, data, &nonce).await),
        Command::StickerPacks => match rest.sticker_packs().await {
            Ok(packs) => NetEvent::StickerPacks(Some(packs)),
            Err(e) if e.is::<rest::Unauthorized>() => NetEvent::AuthExpired,
            Err(e) => {
                tracing::warn!("sticker packs: {}", rest::describe(&e));
                NetEvent::StickerPacks(None)
            }
        },
        Command::Gifs { query } => match rest.gifs(query.as_deref()).await {
            Err(e) if e.is::<rest::Unauthorized>() => NetEvent::AuthExpired,
            gifs => {
                // Counts only: the search is the user's own words.
                match &gifs {
                    Ok(gifs) => tracing::info!(
                        "gifs: {} results, {} with an image to show",
                        gifs.len(),
                        gifs.iter().filter(|gif| gif.image_url().is_some()).count()
                    ),
                    Err(e) => tracing::warn!("gifs: {}", rest::describe(e)),
                }
                NetEvent::Gifs {
                    query,
                    gifs: gifs.map_err(|e| rest::describe(&e)),
                }
            }
        },
        Command::CreateThread {
            parent,
            message,
            name,
            kind,
            auto_archive,
        } => match with_captcha!(|captcha| rest
            .create_thread(parent, message, &name, kind, auto_archive, captcha)
            .await)
        {
            Ok(thread) => NetEvent::ThreadCreated(thread),
            Err(e) => failure(Action::CreateThread, e),
        },
        Command::CreatePost {
            forum,
            name,
            tags,
            content,
            auto_archive,
            files,
        } => 'post: {
            // As with messages, files go up once and a captcha retry only
            // resends the post.
            let uploaded = match rest.upload_files(forum, &files).await {
                Ok(uploaded) => uploaded,
                Err(e) => break 'post failure(Action::CreateThread, e),
            };
            match with_captcha!(|captcha| rest
                .create_post(forum, &name, &tags, auto_archive, &Outgoing { content: &content, files: &uploaded, ..Outgoing::default() }, captcha)
                .await)
            {
                Ok(post) => NetEvent::ThreadCreated(post),
                Err(e) => failure(Action::CreateThread, e),
            }
        }
        Command::ThreadMembership { thread, join, forum_post } => match rest
            .thread_membership(thread, join, forum_post)
            .await
        {
            Ok(()) => NetEvent::ThreadMembership { thread, joined: join },
            Err(e) => failure(Action::ThreadMembership, e),
        },
        Command::EditMessage {
            channel,
            message,
            content,
        } => outcome(Action::Message, rest.edit_message(channel, message, &content).await),
        Command::DeleteMessage { channel, message } => {
            outcome(Action::Message, rest.delete_message(channel, message).await)
        }
        Command::PinMessage { channel, message, pin } => {
            outcome(Action::Message, rest.pin_message(channel, message, pin).await)
        }
        Command::React {
            channel,
            message,
            emoji,
            add,
            source,
        } => {
            let result = if add {
                rest.add_reaction(channel, message, &emoji, source).await
            } else {
                rest.remove_reaction(channel, message, &emoji, source).await
            };
            match result {
                Ok(()) => return,
                Err(e) if e.is::<rest::Unauthorized>() => NetEvent::AuthExpired,
                Err(e) => NetEvent::ReactionFailed {
                    channel,
                    message,
                    emoji,
                    add,
                    error: rest::describe(&e),
                },
            }
        }
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
                tracing::warn!("background DM with {user}: {e:#}");
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
                Err(e) if !e.is::<rest::Unauthorized>() => Some(format!("{e:#}")),
                _ => None,
            },
        },
        Command::Typing { channel } => {
            if let Err(e) = rest.typing(channel).await {
                tracing::debug!("typing in {channel}: {e:#}");
            }
            return;
        }
        Command::FetchProfile { user, guild } => match rest.profile(user, guild).await {
            Ok(profile) => NetEvent::Profile { guild, profile },
            Err(e) => {
                tracing::warn!("profile of {user}: {e:#}");
                return;
            }
        },
        Command::FetchAvatar { url } => {
            let bytes = rest
                .cdn_image(&url)
                .await
                .map_err(|e| tracing::warn!("avatar {url}: {e:#}"))
                .ok();
            NetEvent::Avatar { url, bytes }
        }
        Command::Ack { channel, message } => {
            if let Err(e) = rest.ack(channel, message).await {
                tracing::debug!("ack {channel}/{message}: {e:#}");
            }
            return;
        }
        // Routed to the gateway before spawning.
        Command::SetStatus(_)
        | Command::SetLocale(_)
        | Command::RequestMembers { .. }
        | Command::SearchMembers { .. }
        | Command::QueryMembers { .. }
        | Command::SubscribeGuild { .. } => {
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
        error: rest::describe(&error),
    }
}
