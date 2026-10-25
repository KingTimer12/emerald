//! Network events, each handed to the stores it concerns.

mod handlers;

use std::collections::HashMap;
use std::time::Instant;

use gpui::{Context, Window};

use super::state::{DMS, Space, typing};
use super::{Emerald, Feedback, Overlay, shown};
use super::state::autocomplete::Trigger;
use emerald_discord::model::{Channel, ChannelKind, MembersChunk, Message, TypingStart};
use emerald_discord::{Action, Command, Link, NetEvent, Snapshot};

impl Emerald {
    pub fn on_net(&mut self, event: NetEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            NetEvent::Status(link) => self.connection.set_link(link),
            NetEvent::Fatal(reason) => self.connection.fatal = Some(reason.into()),
            // Intercepted in `main`, which restarts into the login flow.
            NetEvent::AuthExpired | NetEvent::LoggedOut { .. } => {}
            NetEvent::Ready(snapshot) => self.on_ready(*snapshot, window, cx),
            NetEvent::Presences(presences) => self.people.set_presences(presences),
            NetEvent::RelationshipAdd(relationship) => {
                let user = relationship.id;
                // A new friend (either side accepted) gets a DM in the list
                // right away, opened quietly in the background.
                if self.people.add_relationship(relationship) && self.spaces.dm_with(user).is_none() {
                    self.net.post(Command::OpenDm { user, select: false });
                }
            }
            NetEvent::RelationshipRemove(user) => {
                self.people.remove_relationship(user);
                self.friends.forget(user);
            }
            NetEvent::UserUpdate(user) => self.people.update(user),
            NetEvent::DmOpened { mut channel, select } => {
                self.people.adopt_dm(&mut channel);
                let id = channel.id;
                self.spaces.dms_mut().upsert_channel(channel);
                if select {
                    self.spaces.select_dms();
                    self.select_channel(id, window, cx);
                }
            }
            NetEvent::Avatar { url, bytes } => self.images.store(url, bytes),
            NetEvent::Animation { url, frames } => self.images.store_animation(url, frames),
            NetEvent::MembersChunk(chunk) => self.on_members(chunk, cx),
            NetEvent::MemberList(update) => {
                self.members.apply(update);
            }
            NetEvent::MessageAck(ack) => self.reads.acked(ack.channel_id, ack.message_id, ack.mention_count),
            NetEvent::MemberUpdate(update) => {
                let (guild, user, member) = update.into_member();
                if let Some(user) = &member.user {
                    self.people.remember_new(user);
                }
                if let Some(space) = self.spaces.get_mut(guild) {
                    space.update_member(user, member);
                }
                self.refresh_composer(cx);
            }
            NetEvent::RoleUpdate(update) => {
                if let Some(space) = self.spaces.get_mut(update.guild_id) {
                    space.upsert_role(update.role);
                }
                self.refresh_composer(cx);
            }
            NetEvent::RoleDelete(delete) => {
                if let Some(space) = self.spaces.get_mut(delete.guild_id) {
                    space.remove_role(delete.role_id);
                }
                self.refresh_composer(cx);
            }
            NetEvent::ActionDone(action) => self.on_action_result(action, Ok(()), window, cx),
            NetEvent::ActionFailed { action, error } => {
                self.on_action_result(action, Err(error), window, cx)
            }
            NetEvent::GuildCreate(guild) => {
                let added = self.spaces.upsert_guild(guild, self.people.my_id());
                if let Some(ix) = added.filter(|_| std::mem::take(&mut self.select_new_guild)) {
                    self.select_space(ix, window, cx);
                }
            }
            NetEvent::GuildDelete(id) => self.spaces.remove(id),
            NetEvent::ChannelCreate(channel) => self.on_channel(channel),
            NetEvent::ChannelDelete(channel) => {
                if let Some(space) = self.spaces.get_mut(channel.guild_id.unwrap_or(DMS)) {
                    space.remove_channel(channel.id);
                }
                let view = shown(&self.messages, self.selected_channel, channel.id);
                self.history.forget(channel.id, view);
            }
            NetEvent::ThreadListSync(sync) => {
                if let Some(space) = self.spaces.get_mut(sync.guild_id) {
                    space.sync_threads(sync);
                }
                self.refresh_composer(cx);
            }
            NetEvent::ThreadJoined(entry) => {
                let guild = entry.guild_id.unwrap_or(DMS);
                if let Some(space) = self.spaces.get_mut(guild) {
                    space.set_thread_member(entry.id, Some(entry.member()));
                }
                self.refresh_composer(cx);
            }
            NetEvent::ThreadMembers(update) => {
                let me = self.people.my_id();
                let joined = update.added_members.iter().find(|m| m.user_id == Some(me)).map(|m| m.member());
                let left = update.removed_member_ids.contains(&me);
                if let Some(space) = self.spaces.get_mut(update.guild_id)
                    && (joined.is_some() || left)
                {
                    space.set_thread_member(update.id, joined);
                }
                self.refresh_composer(cx);
            }
            NetEvent::ThreadCreated(thread) => self.on_thread_created(thread, window, cx),
            NetEvent::ThreadMembership { thread, joined } => {
                self.on_thread_membership(thread, joined);
                self.refresh_composer(cx);
            }
            NetEvent::ForumPosts { forum, first } => self.on_forum_posts(forum, first),
            NetEvent::ArchivedPosts { forum, offset, page } => self.on_archived_posts(forum, offset, page),
            NetEvent::MessageCreate(message) => self.on_message(*message, cx),
            NetEvent::MessageUpdate(update) => {
                let view = shown(&self.messages, self.selected_channel, update.channel_id);
                self.history.edit(update, view);
            }
            NetEvent::Reaction(event) => self.on_reaction(event),
            NetEvent::PollVote { vote, add } => {
                let mine = vote.user_id == self.people.my_id();
                let view = shown(&self.messages, self.selected_channel, vote.channel_id);
                self.history.update(vote.channel_id, vote.message_id, view, |msg| {
                    msg.poll_vote(vote.answer_id, mine, add)
                });
            }
            NetEvent::ReactionFailed {
                channel,
                message,
                emoji,
                add,
                error,
            } => self.reaction_failed(channel, message, &emoji, add, error),
            NetEvent::GuildEmojis(update) => {
                if let Some(guild) = self.spaces.guild_mut(update.guild_id) {
                    guild.set_emojis(update.emojis);
                }
            }
            NetEvent::GuildStickers(update) => {
                if let Some(guild) = self.spaces.guild_mut(update.guild_id) {
                    guild.set_stickers(update.stickers);
                }
            }
            NetEvent::StickerPacks(packs) => {
                // A failure leaves them to be asked for on the next visit.
                self.sticker_packs = packs;
            }
            NetEvent::Gifs { query, gifs } => self.on_gifs(query, gifs, cx),
            NetEvent::CommandIndex { target, index } => self.on_command_index(target, index, cx),
            NetEvent::MessageDelete(delete) => {
                self.message_gone(delete.channel_id, delete.id, window, cx);
                let view = shown(&self.messages, self.selected_channel, delete.channel_id);
                self.history.delete(delete, view);
            }
            NetEvent::History { channel, page, messages } => {
                let view = shown(&self.messages, self.selected_channel, channel);
                self.history.page(channel, page, messages, view);
                self.request_authors(channel);
                self.continue_jump(channel, cx);
            }
            NetEvent::HistoryFailed {
                channel,
                forbidden,
                error,
            } => {
                let view = shown(&self.messages, self.selected_channel, channel);
                self.history.page_failed(channel, forbidden, view);
                if self.jump.is_some_and(|jump| jump.channel == channel) {
                    self.jump = None;
                }
                self.connection.set_message(t!("common.error", error = error));
            }
            NetEvent::TypingStart(typing) => self.on_typing(typing, cx),
            NetEvent::Profile { guild, profile } => {
                self.profiles.fill((profile.user.id, guild), profile, Instant::now())
            }
            NetEvent::SendFailed {
                channel,
                nonce,
                content,
                files,
                error,
            } => self.on_send_failed(channel, &nonce, content, files, error, cx),
        }
    }
}
