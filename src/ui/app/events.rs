//! Network events, each handed to the stores it concerns.

use std::collections::HashMap;
use std::time::Instant;

use gpui::{Context, Window};

use super::state::{DMS, Space, typing};
use super::{Emerald, Feedback, Overlay, shown};
use super::state::autocomplete::Trigger;
use crate::discord::model::{Channel, ChannelKind, MembersChunk, Message, TypingStart};
use crate::discord::{Action, Command, Link, NetEvent, Snapshot};

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
            NetEvent::MessageCreate(message) => self.on_message(message, cx),
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

    fn on_ready(&mut self, snapshot: Snapshot, window: &mut Window, cx: &mut Context<Self>) {
        let Snapshot {
            me,
            guilds,
            mut dms,
            users,
            relationships,
            members,
            read_states,
        } = snapshot;
        self.connection.set_link(Link::Connected);
        let me_id = me.id;
        self.people.load(me, users, relationships);
        dms.retain(|c| c.kind.is_private());
        for dm in &mut dms {
            self.people.adopt_dm(dm);
        }
        let mut my_roles: HashMap<_, _> = members
            .into_iter()
            .map(|(guild, member)| (guild, member.roles))
            .collect();
        let guilds = guilds.into_iter().filter(|g| !g.unavailable).map(|guild| {
            let roles = my_roles.remove(&guild.id).unwrap_or_default();
            Space::guild(guild, me_id, roles)
        });
        self.spaces.reset(std::iter::once(Space::dms(dms)).chain(guilds).collect());
        self.reads
            .load(read_states, self.spaces.iter().flat_map(Space::readable));
        // A new session (not a resume) may have missed messages and holds
        // no subscriptions: drop cached history and the member list, and
        // refetch only what is on screen.
        self.history.clear();
        self.members.clear();
        let current = self.spaces.current();
        if !current.is_dms() {
            let guild = current.id;
            self.spaces.subscribe(guild);
            self.net.post(Command::SubscribeGuild { guild, members: None });
        }
        if let Some(channel) = self.selected_channel.filter(|_| !self.friends_open) {
            self.select_channel(channel, window, cx);
        }
    }

    fn on_members(&mut self, chunk: MembersChunk, cx: &mut Context<Self>) {
        let found: Vec<_> = chunk.members.iter().filter_map(|m| m.id()).collect();
        self.member_search.accept(chunk.nonce.as_deref(), found.into_iter());
        for member in chunk.members {
            if let Some(user) = &member.user {
                self.people.remember_new(user);
            }
            if let (Some(id), Some(guild)) = (member.id(), self.spaces.guild_mut(chunk.guild_id)) {
                guild.add_member(id, member);
            }
        }
        // An answer to the `@` suggestions' own query, or members that match.
        if self.autocomplete.query().is_some_and(|q| q.trigger == Trigger::Mention) {
            self.refresh_suggestions(true, cx);
        }
    }

    fn on_channel(&mut self, mut channel: Channel) {
        match channel.guild_id {
            Some(guild) => {
                if let Some(space) = self.spaces.get_mut(guild) {
                    space.upsert_channel(channel);
                }
            }
            None => {
                self.people.adopt_dm(&mut channel);
                self.spaces.dms_mut().upsert_channel(channel);
            }
        }
    }

    fn on_message(&mut self, mut message: Message, cx: &mut Context<Self>) {
        let channel = message.channel_id;
        // The REST answer to our own send has no `guild_id`; without one the
        // message would read as a DM and list the channel among the DMs.
        if message.guild_id.is_none() {
            message.guild_id = self.spaces.guild_id_of(channel).or_else(|| {
                // A forum post opened from its forum need not be listed.
                let space = self.spaces.current();
                (self.selected_channel == Some(channel) && !space.is_dms()).then_some(space.id)
            });
        }
        let author = message.author.id;
        let mine = self.people.is_me(author);
        match message.guild_id {
            None => self.touch_dm(&message, mine),
            Some(guild) => {
                if let (Some(member), Some(info)) = (message.member.clone(), self.spaces.guild_mut(guild)) {
                    info.add_member(author, member);
                }
                if let Some(space) = self.spaces.get_mut(guild) {
                    space.touch(channel, message.id);
                }
            }
        }
        if !mine {
            if self.selected_channel == Some(channel) && !self.friends_open {
                self.ack(channel, message.id, cx);
            } else if self.pings_me(&message) {
                self.reads.add_mention(channel);
            } else {
                self.reads.mark_unread(channel);
            }
        }
        self.typing.stop(channel, author);
        let view = shown(&self.messages, self.selected_channel, channel);
        self.history.receive(message, mine, view);
    }

    /// Whether `message` counts as a mention: every DM message does, as on
    /// Discord; in a guild, one naming the user, one of their roles, or
    /// @everyone.
    fn pings_me(&self, message: &Message) -> bool {
        let me = self.people.my_id();
        let Some(guild) = message.guild_id else { return true };
        let my_roles = self.spaces.guild(guild).map_or(&[][..], |g| g.my_roles());
        message.mention_everyone
            || message.mentions.iter().any(|u| u.id == me)
            || message.mention_roles.iter().any(|role| my_roles.contains(role))
    }

    /// Moves the DM a message landed in to the top of the sidebar, adding
    /// it first when the conversation was closed or never opened here.
    fn touch_dm(&mut self, message: &Message, mine: bool) {
        if self.spaces.dms_mut().touch(message.channel_id, message.id) {
            return;
        }
        let mut channel = Channel {
            id: message.channel_id,
            kind: ChannelKind::DM,
            last_message_id: Some(message.id),
            // From someone else, the author is the other side of the DM.
            recipient_ids: if mine { Vec::new() } else { vec![message.author.id] },
            ..Channel::default()
        };
        if !mine {
            self.people.remember(message.author.clone());
        }
        self.people.adopt_dm(&mut channel);
        self.spaces.dms_mut().upsert_channel(channel);
    }

    fn on_typing(&mut self, typing: TypingStart, cx: &mut Context<Self>) {
        if self.people.is_me(typing.user_id) {
            return;
        }
        // Guild typists are often strangers; their member names them.
        if let (Some(guild), Some(mut member)) = (typing.guild_id, typing.member) {
            if let Some(user) = member.user.take() {
                self.people.remember_new(&user);
            }
            if let Some(info) = self.spaces.guild_mut(guild) {
                info.add_member(typing.user_id, member);
            }
        }
        self.typing.start(typing.channel_id, typing.user_id, Instant::now());
        // Redraw when it expires so the indicator goes away on its own.
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(typing::TIMEOUT).await;
            this.update(cx, |this, cx| {
                this.typing.prune(Instant::now());
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn on_action_result(
        &mut self,
        action: Action,
        result: Result<(), String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.busy.remove(&action);
        match action {
            Action::FriendRequest => {
                let username = self.friends.pending.take().unwrap_or_default();
                if result.is_ok() {
                    self.friends.input.update(cx, |input, cx| input.set_text("", cx));
                }
                self.friends.feedback = Some(Feedback::from_result(
                    result,
                    t!("friends.request_sent", username = username),
                ));
            }
            Action::Relationship => {
                if let Err(error) = result {
                    self.connection.set_message(t!("common.error", error = error));
                }
            }
            Action::JoinGuild | Action::CreateGuild => match result {
                Ok(()) => self.close_overlay(window, cx),
                Err(error) => {
                    self.select_new_guild = false;
                    if let Some(Overlay::AddServer(modal)) = &mut self.overlay {
                        modal.error = Some(error.into());
                    }
                }
            },
            Action::Message => {
                if let Err(error) = result {
                    self.composer.error = Some(error.into());
                }
            }
            Action::CreateThread => {
                if let Err(error) = result {
                    self.creating_thread_failed(error);
                }
            }
            Action::CreatePoll => match result {
                Ok(()) => {
                    if matches!(self.overlay, Some(Overlay::CreatePoll(_))) {
                        self.close_overlay(window, cx);
                    }
                }
                Err(error) => match &mut self.overlay {
                    Some(Overlay::CreatePoll(modal)) => modal.error = Some(error.into()),
                    _ => self.composer.error = Some(error.into()),
                },
            },
            Action::ThreadMembership | Action::RunCommand => {
                if let Err(error) = result {
                    self.composer.error = Some(error.into());
                }
            }
            Action::UpdateProfile => {
                if let Some(Overlay::Settings(view)) = &mut self.overlay {
                    view.feedback = Some(Feedback::from_result(result, t!("settings.display_name_saved")));
                }
            }
        }
    }
}
