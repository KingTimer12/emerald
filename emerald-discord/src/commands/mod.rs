//! Runs UI commands against the REST API and turns each outcome into the
//! event the UI waits for. [`handle`] only routes; each area lives in its
//! own module, and [`outcome`] holds the error handling they share.

mod account;
mod app;
mod call;
mod captcha;
mod channel;
mod guild;
mod media;
mod message;
mod moderation;
mod outcome;
mod search;
mod settings;
mod thread;
mod user;

use std::sync::Arc;

use super::packets;
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
        Command::ThreadSearch { channel, query, offset } => Some(thread::search(rest, channel, query, offset).await),
        Command::Pins { channel, before } => Some(search::pins(rest, channel, before).await),
        Command::Search { scope, query, offset } => Some(search::messages(rest, scope, query, offset).await),
        Command::Mentions { before } => Some(search::mentions(rest, before).await),
        Command::Unread { channel, after } => Some(search::unread(rest, channel, after).await),
        Command::GuildSettings { guild, change } => Some(settings::guild(rest, guild, change).await),
        Command::AckBulk { channels } => settings::ack_bulk(rest, channels).await,
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
        Command::Block { user, block } => Some(user::block(rest, user, block).await),
        Command::Ignore { user, ignore } => Some(user::ignore(rest, user, ignore).await),
        Command::SetNickname { user, nickname } => Some(user::set_nickname(rest, user, nickname).await),
        Command::FetchNote { user } => user::note(rest, user).await,
        Command::SetNote { user, note } => Some(user::set_note(rest, user, note).await),
        Command::MutualFriends { user } => Some(user::mutual_friends(rest, user).await),

        Command::CreateGroup { users } => Some(channel::create_group(rest, users).await),
        Command::AddRecipients { channel, users } => Some(channel::add_recipients(rest, channel, users).await),
        Command::RemoveRecipient { channel, user } => Some(channel::remove_recipient(rest, channel, user).await),
        Command::RenameGroup { channel, name } => {
            Some(channel::edit_group(rest, channel, packets::user::group_name(name.as_deref())).await)
        }
        Command::SetGroupIcon { channel, icon } => {
            let uri = icon.map(|(mime, bytes)| packets::user::data_uri(mime, &bytes));
            Some(channel::edit_group(rest, channel, packets::user::group_icon(uri.as_deref())).await)
        }
        Command::CloseChannel { channel } => Some(channel::close(rest, channel).await),
        Command::AnswerRequest { channel, accept } => Some(channel::answer_request(rest, channel, accept).await),
        Command::OpenDm { user, select } => user::open_dm(rest, user, select).await,
        Command::SetGlobalName { name } => Some(user::set_global_name(rest, name, &events).await),
        Command::FetchProfile { user, guild, full } => user::profile(rest, user, guild, full).await,
        Command::FetchProfileEffect { sku } => Some(user::profile_effect(rest, sku).await),
        Command::EditSettings { current, edit } => Some(account::edit_settings(rest, current, edit, &events).await),
        Command::EditProfile { guild, edit } => Some(account::edit_profile(rest, guild, edit, &events).await),
        Command::Sessions => Some(account::sessions(rest).await),
        Command::LogOutSessions(hashes) => Some(account::log_out_sessions(rest, hashes).await),
        Command::LogOut => Some(user::log_out(rest).await),

        Command::JoinGuild { code, message } => Some(guild::join(rest, code, message).await),
        Command::CreateGuild { name } => Some(guild::create(rest, name).await),
        Command::CreateInvite { channel, location } => Some(guild::create_invite(rest, channel, location).await),
        Command::ResolveInvite { code } => Some(guild::resolve_invite(rest, code).await),
        Command::LeaveGuild { guild } => Some(guild::leave(rest, guild).await),

        Command::BulkDelete { channel, messages } => Some(moderation::bulk_delete(rest, channel, messages).await),
        Command::RingCall { channel } => Some(call::ring(rest, channel).await),
        Command::StopRinging { channel, recipients } => Some(call::stop_ringing(rest, channel, recipients).await),
        Command::Kick { guild, user } => Some(moderation::kick(rest, guild, user).await),
        Command::Ban { guild, user, delete_seconds } => Some(moderation::ban(rest, guild, user, delete_seconds).await),
        Command::Timeout { guild, user, until } => Some(moderation::timeout(rest, guild, user, until).await),
        Command::EditChannel { channel, edit } => Some(moderation::edit_channel(rest, channel, edit).await),

        // Routed to the gateway before spawning.
        Command::SetPresence(_)
        | Command::SetLocale(_)
        | Command::RequestMembers { .. }
        | Command::SearchMembers { .. }
        | Command::QueryMembers { .. }
        | Command::SubscribeGuild { .. }
        | Command::VoiceState { .. }
        | Command::VoiceConnect(_)
        | Command::VoiceDisconnect
        | Command::VoiceControls(_) => None,
    };
    if let Some(event) = event {
        events.try_send(event).ok();
    }
}
