//! Runs UI commands against the REST API and turns each outcome into the
//! event the UI waits for. [`handle`] only routes; each area lives in its
//! own module, and [`outcome`] holds the error handling they share.

mod app;
mod captcha;
mod guild;
mod media;
mod message;
mod outcome;
mod thread;
mod user;

use std::sync::Arc;

use super::protocol::{Command, NetEvent};
use super::rest::Rest;

/// Where commands report back.
type Events = async_channel::Sender<NetEvent>;

/// Runs `command` and sends its event, if it has one.
pub(super) async fn handle(rest: Arc<Rest>, command: Command, events: Events) {
    let rest = &*rest;
    let event = match command {
        Command::History { channel, page } => Some(message::history(rest, channel, page).await),
        Command::Send {
            channel,
            content,
            nonce,
            reply,
            files,
        } => Some(message::send(rest, channel, content, nonce, reply, files).await),
        Command::SendSticker { channel, sticker, nonce } => Some(message::send_sticker(rest, channel, sticker, nonce).await),
        Command::SendGif { channel, gif, query, nonce } => Some(message::send_gif(rest, channel, gif, query, nonce).await),
        Command::SendPoll { channel, nonce, poll } => Some(message::send_poll(rest, channel, nonce, poll).await),
        Command::EditMessage {
            channel,
            message,
            content,
        } => Some(message::edit(rest, channel, message, content).await),
        Command::DeleteMessage { channel, message } => Some(message::delete(rest, channel, message).await),
        Command::VotePoll {
            channel,
            message,
            answers,
        } => Some(message::vote(rest, channel, message, answers).await),
        Command::PinMessage { channel, message, pin } => Some(message::pin(rest, channel, message, pin).await),
        Command::React {
            channel,
            message,
            emoji,
            add,
            source,
        } => message::react(rest, channel, message, emoji, add, source).await,
        Command::Ack { channel, message } => message::ack(rest, channel, message).await,
        Command::Typing { channel } => message::typing(rest, channel).await,

        Command::ForumPosts { forum, threads } => Some(thread::forum_posts(rest, forum, threads).await),
        Command::ArchivedPosts { forum, offset } => Some(thread::archived_posts(rest, forum, offset).await),
        Command::CreateThread {
            parent,
            message,
            name,
            kind,
            auto_archive,
        } => Some(thread::create(rest, parent, message, name, kind, auto_archive).await),
        Command::CreatePost {
            forum,
            name,
            tags,
            content,
            auto_archive,
            files,
        } => Some(thread::create_post(rest, forum, name, tags, content, auto_archive, files).await),
        Command::ThreadMembership { thread, join, forum_post } => {
            Some(thread::membership(rest, thread, join, forum_post).await)
        }

        Command::ListCommands(target) => Some(app::list(rest, target).await),
        Command::Interact {
            application,
            guild,
            channel,
            data,
            nonce,
        } => Some(app::interact(rest, application, guild, channel, data, nonce).await),

        Command::Gifs { query } => Some(media::gifs(rest, query).await),
        Command::StickerPacks => Some(media::sticker_packs(rest).await),
        Command::FetchAvatar { url } => Some(media::avatar(rest, url).await),
        Command::FetchAnimation { url, width, height } => Some(media::animation(rest, url, width, height).await),

        Command::SendFriendRequest { username } => Some(user::friend_request(rest, username).await),
        Command::AcceptFriend { user } => Some(user::accept_friend(rest, user).await),
        Command::RemoveRelationship { user } => Some(user::remove_relationship(rest, user).await),
        Command::OpenDm { user, select } => user::open_dm(rest, user, select).await,
        Command::SetGlobalName { name } => Some(user::set_global_name(rest, name, &events).await),
        Command::FetchProfile { user, guild } => user::profile(rest, user, guild).await,
        Command::LogOut => Some(user::log_out(rest).await),

        Command::JoinGuild { code } => Some(guild::join(rest, code).await),
        Command::CreateGuild { name } => Some(guild::create(rest, name).await),

        // Routed to the gateway before spawning.
        Command::SetStatus(_)
        | Command::SetLocale(_)
        | Command::RequestMembers { .. }
        | Command::SearchMembers { .. }
        | Command::QueryMembers { .. }
        | Command::SubscribeGuild { .. } => None,
    };
    if let Some(event) = event {
        events.try_send(event).ok();
    }
}
