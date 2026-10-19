//! Network events, each handed to the stores it concerns.

use std::collections::HashMap;
use std::time::Instant;

use gpui::{Context, Window};

use super::state::{DMS, Space, typing};
use super::{CONNECTED, Emerald, Feedback, Overlay, shown};
use crate::discord::model::{Channel, ChannelKind, MembersChunk, Message, TypingStart};
use crate::discord::{Action, Command, NetEvent, Snapshot};

impl Emerald {
    pub fn on_net(&mut self, event: NetEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            NetEvent::Status(status) => self.connection.set_status(status),
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
            NetEvent::MembersChunk(chunk) => self.on_members(chunk),
            NetEvent::MessageAck(ack) => self.reads.acked(ack.channel_id, ack.message_id),
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
            NetEvent::MessageCreate(message) => self.on_message(message, cx),
            NetEvent::MessageUpdate(update) => {
                let view = shown(&self.messages, self.selected_channel, update.channel_id);
                self.history.edit(update, view);
            }
            NetEvent::MessageDelete(delete) => {
                let view = shown(&self.messages, self.selected_channel, delete.channel_id);
                self.history.delete(delete, view);
            }
            NetEvent::History {
                channel,
                before,
                messages,
            } => {
                let view = shown(&self.messages, self.selected_channel, channel);
                self.history.page(channel, before, messages, view);
                self.request_authors(channel);
            }
            NetEvent::HistoryFailed {
                channel,
                forbidden,
                error,
            } => {
                let view = shown(&self.messages, self.selected_channel, channel);
                self.history.page_failed(channel, forbidden, view);
                self.connection.status = format!("Erro: {error}").into();
            }
            NetEvent::TypingStart(typing) => self.on_typing(typing, cx),
            NetEvent::Profile(profile) => self.profiles.fill(profile.user.id, profile, Instant::now()),
            NetEvent::SendFailed {
                channel,
                nonce,
                content,
                error,
            } => self.on_send_failed(channel, &nonce, content, error, cx),
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
        self.connection.set_status(CONNECTED.to_owned());
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
        // A new session (not a resume) may have missed messages: drop cached
        // history and refetch only what is on screen.
        self.history.clear();
        let current = self.spaces.current();
        if !current.is_dms() {
            let guild = current.id;
            self.spaces.subscribe(guild);
            self.net.post(Command::SubscribeGuild { guild });
        }
        if let Some(channel) = self.selected_channel.filter(|_| !self.friends_open) {
            self.select_channel(channel, window, cx);
        }
    }

    fn on_members(&mut self, chunk: MembersChunk) {
        for member in chunk.members {
            if let Some(user) = &member.user {
                self.people.remember_new(user);
            }
            if let (Some(id), Some(guild)) = (member.id(), self.spaces.guild_mut(chunk.guild_id)) {
                guild.add_member(id, member);
            }
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

    fn on_message(&mut self, message: Message, cx: &mut Context<Self>) {
        let channel = message.channel_id;
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
            if self.selected_channel == Some(channel) {
                self.ack(channel, message.id, cx);
            } else {
                self.reads.mark_unread(channel);
            }
        }
        self.typing.stop(channel, author);
        let view = shown(&self.messages, self.selected_channel, channel);
        self.history.receive(message, mine, view);
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
                    format!("Sucesso! Seu pedido de amizade para {username} foi enviado."),
                ));
            }
            Action::Relationship => {
                if let Err(error) = result {
                    self.connection.status = format!("Erro: {error}").into();
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
            Action::UpdateProfile => {
                if let Some(Overlay::Settings(view)) = &mut self.overlay {
                    view.feedback = Some(Feedback::from_result(result, "Nome exibido atualizado."));
                }
            }
        }
    }
}
