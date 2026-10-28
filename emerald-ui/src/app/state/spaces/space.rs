//! One space: the DMs or a guild, with its channels and rows.

use super::*;

pub struct Space {
    pub id: Snowflake,
    pub name: SharedString,
    pub initials: SharedString,
    pub icon_url: Option<SharedString>,
    /// Threads among them, under their parent's id.
    pub(super) channels: Vec<Channel>,
    pub(super) rows: Vec<Row>,
    /// `None` for the DM space.
    pub(super) guild: Option<GuildInfo>,
    /// The open thread, listed under its parent even when not joined.
    pub(super) shown_thread: Option<Snowflake>,
    /// DMs: what each is called, from its people's names.
    pub(super) labels: HashMap<Snowflake, SharedString>,
    /// Who is in a voice channel here, in the order they joined.
    pub(super) voice: Vec<VoiceState>,
}

impl Space {
    /// The DM space; `channels` must already be adopted by `People`.
    pub fn dms(mut channels: Vec<Channel>) -> Self {
        channels.retain(|c| c.kind.is_private());
        let mut space = Self {
            id: DMS,
            name: t!("dm.title").into(),
            initials: "DM".into(),
            icon_url: None,
            channels,
            rows: Vec::new(),
            guild: None,
            shown_thread: None,
            labels: HashMap::new(),
            voice: Vec::new(),
        };
        space.rebuild();
        space
    }

    pub fn guild(guild: Guild, me: Snowflake, my_roles: Vec<Snowflake>) -> Self {
        let name = guild.display_name().to_owned();
        let upload_limit = guild.upload_limit();
        let mut space = Self {
            id: guild.id,
            initials: initials(&name).into(),
            icon_url: guild.icon_url().map(Into::into),
            name: name.into(),
            guild: Some(GuildInfo {
                me,
                owner: guild.owner(),
                roles: guild.roles,
                my_roles,
                members: HashMap::new(),
                requested: HashSet::new(),
                permissions: HashMap::new(),
                upload_limit,
                emojis: guild.emojis,
                stickers: guild.stickers,
            }),
            channels: guild.channels,
            rows: Vec::new(),
            shown_thread: None,
            labels: HashMap::new(),
            voice: guild.voice_states.into_iter().filter(|v| v.channel_id.is_some()).collect(),
        };
        for mut thread in guild.threads {
            thread.guild_id = Some(space.id);
            space.channels.push(thread);
        }
        space.rebuild();
        space
    }

    pub fn is_dms(&self) -> bool {
        self.id == DMS
    }

    /// Names the DMs with `label`, which reads their people's names; run
    /// again when those names change.
    pub fn label_dms(&mut self, label: impl Fn(&Channel) -> String) {
        self.labels = self.channels.iter().map(|c| (c.id, label(c).into())).collect();
        self.rebuild();
    }

    /// The DM `id` as the sidebar calls it.
    pub fn label(&self, id: Snowflake) -> Option<&SharedString> {
        self.labels.get(&id)
    }

    /// Message requests waiting (`spam` false) or in the spam inbox.
    pub fn requests(&self, spam: bool) -> impl Iterator<Item = &Channel> {
        self.channels
            .iter()
            .filter(move |c| if spam { c.is_spam_request() } else { c.is_request() })
    }

    pub fn channel_mut(&mut self, id: Snowflake) -> Option<&mut Channel> {
        self.channels.iter_mut().find(|c| c.id == id)
    }

    /// Redraws the rows after a channel was changed in place.
    pub fn refresh(&mut self) {
        self.rebuild();
    }

    pub fn info(&self) -> Option<&GuildInfo> {
        self.guild.as_ref()
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn channels(&self) -> &[Channel] {
        &self.channels
    }

    /// The current user's permissions in `channel`; everything in DMs.
    pub fn permissions(&self, channel: Snowflake) -> u64 {
        match &self.guild {
            Some(guild) => guild.permissions.get(&channel).copied().unwrap_or(0),
            None => u64::MAX,
        }
    }

    /// Server-wide permissions: a channel overwrite can deny a bit there, so
    /// this ors every channel and keeps what any one of them grants. Kick,
    /// ban and timeout are only ever granted server-wide.
    pub fn guild_permissions(&self) -> u64 {
        match &self.guild {
            Some(guild) => guild.permissions.values().fold(0, |granted, bits| granted | bits),
            None => u64::MAX,
        }
    }

    /// Channels the current user can open and see.
    pub fn readable(&self) -> impl Iterator<Item = &Channel> {
        self.channels.iter().filter(|c| {
            c.kind.is_openable() && self.permissions(c.id) & permissions::VIEW_CHANNEL != 0
        })
    }

    pub fn row(&self, id: Snowflake) -> Option<&ChannelRow> {
        self.rows.iter().find_map(|row| match row {
            Row::Channel(channel) if channel.id == id => Some(channel),
            _ => None,
        })
    }

    pub fn first_openable(&self) -> Option<Snowflake> {
        self.rows.iter().find_map(|row| match row {
            Row::Channel(channel) if channel.kind.is_openable() => Some(channel.id),
            _ => None,
        })
    }

    pub fn topic(&self, id: Snowflake) -> Option<&str> {
        self.channels
            .iter()
            .find(|c| c.id == id)
            .and_then(|c| c.topic.as_deref())
            .filter(|t| !t.is_empty())
    }

    /// Indices of the rows whose channel name contains `query`; all rows
    /// for an empty query.
    pub fn search(&self, query: &str) -> Vec<usize> {
        let query = query.trim().to_lowercase();
        self.matching(|row| match row {
            _ if query.is_empty() => true,
            Row::Channel(channel) => channel.name.to_lowercase().contains(&query),
            _ => false,
        })
    }

    /// Indices of the rows outside collapsed categories, plus those `keep`
    /// pins in place (the selected and unread channels, as on Discord).
    pub fn expanded(&self, collapsed: impl Fn(Snowflake) -> bool, keep: impl Fn(Snowflake) -> bool) -> Vec<usize> {
        self.matching(|row| match row {
            Row::Channel(ChannelRow {
                id,
                parent: Some(parent),
                ..
            }) => !collapsed(*parent) || keep(*id),
            Row::Voice(VoiceRow {
                channel,
                parent: Some(parent),
                ..
            }) => !collapsed(*parent) || keep(*channel),
            _ => true,
        })
    }

    pub(super) fn matching(&self, keep: impl Fn(&Row) -> bool) -> Vec<usize> {
        (0..self.rows.len()).filter(|ix| keep(&self.rows[*ix])).collect()
    }

    /// GUILD_MEMBER_UPDATE. The current user's roles also change what they
    /// may see and do.
    pub fn update_member(&mut self, user: Snowflake, member: Member) {
        let Some(guild) = &mut self.guild else { return };
        let mine = user == guild.me;
        if mine {
            guild.my_roles = member.roles.clone();
        }
        guild.members.insert(user, member);
        if mine {
            self.rebuild();
        }
    }

    /// GUILD_ROLE_CREATE and GUILD_ROLE_UPDATE.
    pub fn upsert_role(&mut self, role: Role) {
        let Some(guild) = &mut self.guild else { return };
        match guild.roles.iter_mut().find(|r| r.id == role.id) {
            Some(known) => *known = role,
            None => guild.roles.push(role),
        }
        self.rebuild();
    }

    pub fn remove_role(&mut self, id: Snowflake) {
        let Some(guild) = &mut self.guild else { return };
        guild.roles.retain(|r| r.id != id);
        guild.my_roles.retain(|r| *r != id);
        self.rebuild();
    }

    pub fn upsert_channel(&mut self, mut channel: Channel) {
        if let Some(old) = self.channels.iter().position(|c| c.id == channel.id) {
            let old = self.channels.swap_remove(old);
            // THREAD_UPDATE leaves out our membership; it has not changed.
            if channel.kind.is_thread() && channel.member.is_none() {
                channel.member = old.member;
            }
        }
        self.channels.push(channel);
        self.rebuild();
    }

    /// THREAD_LIST_SYNC: the active threads, with our memberships.
    pub fn sync_threads(&mut self, sync: ThreadListSync) {
        for mut thread in sync.threads {
            thread.guild_id = Some(self.id);
            if let Some(member) = sync.members.iter().find(|m| m.id == thread.id) {
                thread.member = Some(member.member());
            }
            if let Some(old) = self.channels.iter().position(|c| c.id == thread.id) {
                let old = self.channels.swap_remove(old);
                thread.member = thread.member.or(old.member);
            }
            self.channels.push(thread);
        }
        self.rebuild();
    }

    /// Threads loaded from a search, such as archived forum posts; ones
    /// already known keep our membership.
    pub fn add_threads(&mut self, threads: Vec<Channel>) {
        for mut thread in threads {
            thread.guild_id = Some(self.id);
            if let Some(old) = self.channels.iter().position(|c| c.id == thread.id) {
                let old = self.channels.swap_remove(old);
                thread.member = thread.member.or(old.member);
            }
            self.channels.push(thread);
        }
        self.rebuild();
    }

    /// We joined `thread` (`Some`) or left it.
    pub fn set_thread_member(&mut self, thread: Snowflake, member: Option<ThreadMember>) {
        if let Some(found) = self.channels.iter_mut().find(|c| c.id == thread) {
            found.member = member;
            self.rebuild();
        }
    }

    /// Lists `thread` under its parent while it is open.
    pub fn show_thread(&mut self, thread: Option<Snowflake>) {
        if self.shown_thread != thread {
            self.shown_thread = thread;
            self.rebuild();
        }
    }

    pub fn remove_channel(&mut self, id: Snowflake) {
        self.channels.retain(|c| c.id != id);
        self.rebuild();
    }

    /// VOICE_STATE_UPDATE: someone joined, left, moved or changed their
    /// mute. False when nothing shown changed.
    pub fn set_voice_state(&mut self, state: VoiceState) -> bool {
        let before = self.voice.iter().position(|v| v.user_id == state.user_id);
        let shown = |v: &VoiceState| (v.channel_id, v.mute || v.self_mute, v.deaf || v.self_deaf, v.self_stream);
        match (before, state.channel_id) {
            // Same channel: updated in place, keeping their place in the list.
            (Some(ix), Some(channel)) if self.voice[ix].channel_id == Some(channel) => {
                let changed = shown(&self.voice[ix]) != shown(&state);
                self.voice[ix] = state;
                if !changed {
                    return false;
                }
            }
            (Some(ix), channel) => {
                self.voice.remove(ix);
                if channel.is_some() {
                    self.voice.push(state);
                }
            }
            (None, Some(_)) => self.voice.push(state),
            (None, None) => return false,
        }
        self.rebuild();
        true
    }

    /// Who is in voice `channel`.
    pub fn voice_in(&self, channel: Snowflake) -> impl Iterator<Item = &VoiceState> {
        self.voice.iter().filter(move |v| v.channel_id == Some(channel))
    }

    /// Everyone in voice here.
    pub fn voice_users(&self) -> impl Iterator<Item = Snowflake> + '_ {
        self.voice.iter().map(|v| v.user_id)
    }

    /// Records `message` as the newest in `channel`; false when the channel
    /// is not here.
    pub fn touch(&mut self, channel: Snowflake, message: Snowflake) -> bool {
        let Some(found) = self.channels.iter_mut().find(|c| c.id == channel) else {
            return false;
        };
        found.last_message_id = Some(message);
        // Only the DM list is ordered by activity.
        if self.guild.is_none() {
            self.rebuild();
        }
        true
    }
}
