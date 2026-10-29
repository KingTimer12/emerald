//! What each kind of network event does to the stores and the window.

use super::*;
use crate::lottie::Player;

impl Emerald {
    #[hotpath::measure]
    pub(super) fn on_ready(&mut self, snapshot: Snapshot, window: &mut Window, cx: &mut Context<Self>) {
        self.end_call_for_new_session();
        let Snapshot {
            me,
            guilds,
            mut dms,
            users,
            relationships,
            members,
            read_states,
            guild_settings,
            folders,
            settings,
            session_hash,
        } = snapshot;
        self.connection.set_link(Link::Connected);
        let me_id = me.id;
        self.people.load(me, users, relationships);
        self.load_account(settings, session_hash, cx);
        dms.retain(|c| c.kind.is_private());
        for dm in &mut dms {
            self.people.adopt_dm(dm);
        }
        let defaults: Vec<_> = guilds.iter().map(|g| (g.id, g.default_notify())).collect();
        self.notify.load(guild_settings, defaults.into_iter());
        self.server_list.set(folders);
        let mut my_roles: FxHashMap<_, _> = members
            .into_iter()
            .map(|(guild, member)| (guild, member.roles))
            .collect();
        let guilds = guilds.into_iter().filter(|g| !g.unavailable).map(|guild| {
            let roles = my_roles.remove(&guild.id).unwrap_or_default();
            Space::guild(guild, me_id, roles)
        });
        self.spaces.reset(std::iter::once(Space::dms(dms)).chain(guilds).collect());
        self.label_dms();
        self.reorder_spaces();
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
        } else if self.selected_channel.is_none()
            && let Some(channel) = self.settings.last_channel
        {
            // The first READY of a launch: reopen where the last session ended.
            self.restore_channel(channel, window, cx);
        }
        self.follow_pending_link(window, cx);
    }

    #[hotpath::measure]
    pub(super) fn on_members(&mut self, chunk: MembersChunk, cx: &mut Context<Self>) {
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

    pub(super) fn on_channel(&mut self, mut channel: Channel) {
        match channel.guild_id {
            Some(guild) => {
                if let Some(space) = self.spaces.get_mut(guild) {
                    space.upsert_channel(channel);
                }
            }
            None => {
                self.people.adopt_dm(&mut channel);
                self.spaces.dms_mut().upsert_channel(channel);
                self.label_dms();
            }
        }
    }

    #[hotpath::measure]
    pub(super) fn on_message(&mut self, mut message: Message, cx: &mut Context<Self>) {
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
            self.alert_message(&message, self.window_active);
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

    /// A download arrived. Lottie stickers are read on a background
    /// thread: a big one takes over 10 ms, a dropped frame on this one.
    pub(super) fn on_image(&mut self, url: String, bytes: Option<Vec<u8>>, cx: &mut Context<Self>) {
        let sizes = self.images.lottie_sizes(&url);
        if sizes == [None, None] {
            self.images.store(url, bytes);
            return;
        }
        let Some(bytes) = bytes else {
            self.images.store_players(&url, [None, None]);
            return;
        };
        cx.spawn(async move |this, cx| {
            let players = cx
                .background_executor()
                .spawn(async move { sizes.map(|size| size.and_then(|size| Player::new(&bytes, size))) })
                .await;
            this.update(cx, |this, cx| {
                this.images.store_players(&url, players);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Whether `message` counts as a mention: every DM message does, as on
    /// Discord; in a guild, one naming the user, one of their roles, or
    /// @everyone.
    pub(super) fn pings_me(&self, message: &Message) -> bool {
        let me = self.people.my_id();
        let Some(guild) = message.guild_id else { return true };
        let my_roles = self.spaces.guild(guild).map_or(&[][..], |g| g.my_roles());
        message.mention_everyone
            || message.mentions.iter().any(|u| u.id == me)
            || message.mention_roles.iter().any(|role| my_roles.contains(role))
    }

    /// Moves the DM a message landed in to the top of the sidebar, adding
    /// it first when the conversation was closed or never opened here.
    pub(super) fn touch_dm(&mut self, message: &Message, mine: bool) {
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
        self.label_dms();
    }

    #[hotpath::measure]
    pub(super) fn on_typing(&mut self, typing: TypingStart, cx: &mut Context<Self>) {
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

    pub(super) fn on_action_result(
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
            Action::Settings | Action::Channel | Action::Note | Action::LeaveGuild | Action::Moderation | Action::Call => {
                if let Err(error) = result {
                    self.connection.set_message(t!("common.error", error = error));
                }
            }
            Action::ChannelSettings => match result {
                Ok(()) => {
                    if matches!(self.overlay, Some(Overlay::ChannelSettings(_))) {
                        self.close_overlay(window, cx);
                    }
                }
                Err(error) => match &mut self.overlay {
                    Some(Overlay::ChannelSettings(modal)) => modal.error = Some(error.into()),
                    _ => self.connection.set_message(t!("common.error", error = error)),
                },
            },
            Action::ThreadMembership | Action::RunCommand => {
                if let Err(error) = result {
                    self.composer.error = Some(error.into());
                }
            }
            Action::UpdateProfile => {
                if result.is_ok() {
                    self.profile_saved(cx);
                }
                if let Some(Overlay::Settings(view)) = &mut self.overlay {
                    view.feedback = Some(Feedback::from_result(result, t!("settings.profile_saved")));
                }
            }
            Action::AccountSettings | Action::Sessions => {
                if let Err(error) = result {
                    match &mut self.overlay {
                        Some(Overlay::Settings(view)) => view.feedback = Some(Feedback::from_result(Err(error), "")),
                        _ => self.connection.set_message(t!("common.error", error = error)),
                    }
                }
            }
        }
    }
}
