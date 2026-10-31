//! Network events, each handed to the stores it concerns.

mod handlers;

use std::time::Instant;

use rustc_hash::FxHashMap;

use gpui::{Context, Window};

use super::state::{DMS, Space, typing};
use super::{Emerald, Feedback, Overlay, shown};
use super::state::autocomplete::Trigger;
use emerald_discord::model::{Channel, ChannelKind, MembersChunk, Message, MessageDelete, TypingStart};
use emerald_discord::{Action, Command, Link, NetEvent, Snapshot};

impl Emerald {
    #[hotpath::measure]
    pub fn on_net(&mut self, event: NetEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            NetEvent::Status(link) => self.connection.set_link(link),
            NetEvent::Fatal(reason) => self.connection.fatal = Some(reason.into()),
            // Intercepted in `main`, which restarts into the login flow.
            NetEvent::AuthExpired | NetEvent::LoggedOut { .. } => {}
            NetEvent::Ready(snapshot) => self.on_ready(*snapshot, window, cx),
            NetEvent::Presences(presences) => self.people.set_presences(presences),
            NetEvent::VoiceState(state) => self.on_voice_state(state, cx),
            NetEvent::VoiceServer(server) => self.on_voice_server(server),
            NetEvent::Voice(event) => self.on_voice_event(event, cx),
            NetEvent::Call(update) => self.on_call(update, cx),
            NetEvent::CallDelete(channel) => self.on_call_delete(channel, cx),
            NetEvent::RelationshipAdd(relationship) => {
                let user = relationship.id;
                // A new friend (either side accepted) gets a DM in the list
                // right away, opened quietly in the background.
                if self.people.add_relationship(relationship) && self.spaces.dm_with(user).is_none() {
                    self.net.post(Command::OpenDm { user, select: false });
                }
                self.label_dms();
                self.redraw_chat();
            }
            NetEvent::RelationshipRemove(user) => {
                self.people.remove_relationship(user);
                self.friends.forget(user);
                self.label_dms();
                self.redraw_chat();
            }
            NetEvent::Recipient { channel, user, added } => self.on_recipient(channel, user, added),
            NetEvent::Note { user, note } => self.on_note(user, note, window, cx),
            NetEvent::MutualFriends { user, friends } => {
                let friends = friends.unwrap_or_default();
                let ids = friends.iter().map(|u| u.id).collect();
                for friend in friends {
                    self.people.remember_new(&friend);
                }
                self.mutual_friends.insert(user, ids);
            }
            NetEvent::InviteCreated { channel, invite } => self.on_invite_created(channel, invite),
            NetEvent::Invite { code, invite } => self.invite_cards.store(code, invite),
            NetEvent::UserUpdate(user) => {
                if self.people.is_me(user.id) {
                    super::account::remember_account(&user);
                }
                self.people.update(user);
            }
            NetEvent::UserSettings { settings, partial } => self.on_user_settings(settings, partial, cx),
            NetEvent::ProfileEdited { guild, details } => self.on_profile_edited(guild, details),
            NetEvent::Sessions(sessions) => self.account.set_sessions(sessions),
            NetEvent::SessionsLoggedOut(hashes) => {
                self.busy.remove(&Action::Sessions);
                self.account.logged_out(&hashes);
            }
            NetEvent::DmOpened { mut channel, select } => {
                self.people.adopt_dm(&mut channel);
                let id = channel.id;
                self.spaces.dms_mut().upsert_channel(channel);
                self.label_dms();
                if select {
                    self.requests_open = false;
                    self.spaces.select_dms();
                    self.select_channel(id, window, cx);
                }
            }
            NetEvent::Avatar { url, bytes } => self.on_image(url, bytes, cx),
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
                let id = guild.id;
                self.notify.set_default(id, guild.default_notify());
                let added = self.spaces.upsert_guild(guild, self.people.my_id());
                // A new server takes its place in the user's order.
                let added = added.and_then(|_| {
                    self.reorder_spaces();
                    self.spaces.index(id)
                });
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
            NetEvent::ThreadSearch {
                channel,
                query,
                offset,
                mut page,
            } => {
                if !self.on_thread_panel_page(channel, &query, offset, &mut page) {
                    self.on_forum_page(channel, query, offset, page);
                }
            }
            NetEvent::Pins { channel, before, page } => self.on_pins(channel, before, page),
            NetEvent::SearchResults {
                scope,
                query,
                offset,
                page,
            } => self.on_search_results(scope, query, offset, page),
            NetEvent::Mentions { before, messages } => self.on_mentions(before, messages),
            NetEvent::Unread { channel, messages } => self.on_unread(channel, messages),
            NetEvent::GuildSettings(settings) => self.notify.update(settings),
            NetEvent::Folders(folders) => {
                self.server_list.set(Some(folders));
                self.reorder_spaces();
            }
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
            NetEvent::MessageDeleteBulk(delete) => {
                for id in delete.ids {
                    self.message_gone(delete.channel_id, id, window, cx);
                    let view = shown(&self.messages, self.selected_channel, delete.channel_id);
                    self.history.delete(MessageDelete { id, channel_id: delete.channel_id }, view);
                }
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
            NetEvent::ProfileEffect { sku, effect } => {
                self.profile_effects.insert(sku, effect.map(std::rc::Rc::new));
            }
            NetEvent::Profile { guild, mut profile } => {
                let key = (profile.user.id, guild);
                if let Some(user) = profile.user.user() {
                    self.people.remember_new(&user);
                }
                // The DM panel's copy leaves out what the full one had.
                if profile.mutual_guilds.is_none()
                    && let Some(old) = self.profiles.get(&key)
                {
                    profile.mutual_guilds = old.mutual_guilds.clone();
                    profile.mutual_friends_count = old.mutual_friends_count;
                }
                self.ask_profile_effect(&profile);
                self.profiles.fill(key, *profile, Instant::now());
                if self.people.is_me(key.0) {
                    self.fill_profile_form(cx);
                }
            }
            NetEvent::SendFailed {
                channel,
                nonce,
                content,
                files,
                error,
            } => self.on_send_failed(channel, &nonce, content, files, error, cx),
        }
        self.refresh_badge();
    }

    /// Names the DMs after their people again, after a name, nickname or
    /// member changed.
    pub(super) fn label_dms(&mut self) {
        let people = &self.people;
        self.spaces.dms_mut().label_dms(|channel| people.dm_name(channel));
    }

    /// Redraws the open chat, whose blocked and ignored messages fold
    /// differently once a relationship changed.
    fn redraw_chat(&self) {
        if let Some(channel) = self.selected_channel {
            self.history.redraw_all(channel, shown(&self.messages, self.selected_channel, channel));
        }
    }

    /// Someone joined or left a group DM; the user leaving drops it.
    fn on_recipient(&mut self, channel: emerald_discord::model::Snowflake, user: emerald_discord::model::User, added: bool) {
        let id = user.id;
        let me = self.people.is_me(id);
        self.people.remember_new(&user);
        let dms = self.spaces.dms_mut();
        if me && !added {
            dms.remove_channel(channel);
        } else if let Some(group) = dms.channel_mut(channel) {
            group.recipient_ids.retain(|r| *r != id);
            if added {
                group.recipient_ids.push(id);
            }
        }
        self.label_dms();
    }

    /// Puts the servers in the user's order, folders flattened.
    pub(super) fn reorder_spaces(&mut self) {
        let order = self.server_list.order(&self.spaces.guild_ids());
        self.spaces.reorder(&order);
    }
}
