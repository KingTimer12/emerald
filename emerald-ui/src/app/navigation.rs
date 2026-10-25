//! Moving around: picking a space or channel, paging history back, and the
//! requests that follow what is on screen (read receipts, members, profiles).

use std::time::{Duration, Instant};

use gpui::{App, Context, Focusable, Window};

use super::Emerald;
use super::chat::JumpTarget;
use super::state::Row;
use emerald_discord::{Command, Page};
use emerald_discord::model::{Channel, Snowflake};
use emerald_discord::permissions;

/// Read receipts wait this long so a burst of messages sends one.
const ACK_DELAY: Duration = Duration::from_millis(1500);
/// The web client asks for at most this many users per op 8.
const MEMBERS_PER_REQUEST: usize = 100;

impl Emerald {
    /// Opens a space: DMs show the friends list; a guild reopens its last
    /// channel, else its first one the user can read, like the web client.
    pub(super) fn select_space(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.spaces.select(ix);
        let space = self.spaces.current();
        if space.is_dms() {
            self.friends_open = true;
            cx.notify();
            return;
        }
        let guild = space.id;
        self.reveal_guild(guild);
        if self.spaces.subscribe(guild) {
            self.net.post(Command::SubscribeGuild { guild, members: None });
        }
        match self.spaces.landing_channel() {
            Some(channel) => self.select_channel(channel, window, cx),
            None => {
                self.selected_channel = None;
                self.members.close();
                cx.notify();
            }
        }
    }

    /// A server in a closed folder opens the folder, as on Discord.
    fn reveal_guild(&mut self, guild: Snowflake) {
        if let Some(folder) = self.server_list.folder_of(guild)
            && !self.server_list.is_open(folder)
        {
            self.server_list.toggle(folder);
        }
    }

    pub(super) fn select_channel(&mut self, id: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_channel != Some(id) {
            // The divider marks what was unread when the channel opened,
            // and stays while it is open, as on Discord.
            self.unread_marker = self.reads.last_read(id).filter(|_| self.reads.is_unread(id)).map(|read| (id, read));
            self.divider_seen = false;
        }
        self.friends_open = false;
        self.selected_channel = Some(id);
        self.spaces.remember_channel(id);
        self.recent.visit(id);
        self.panel_follow(id);
        self.reads.seen(id);
        let kind = self.spaces.channel(id).map(|c| c.kind).unwrap_or_default();
        let current = self.spaces.current().id;
        if let Some(space) = self.spaces.get_mut(current) {
            space.show_thread(kind.is_thread().then_some(id));
        }

        let space = self.spaces.current();
        let readable = space.permissions(id) & permissions::READ_MESSAGE_HISTORY != 0;
        let user = space.row(id).and_then(|row| row.user);
        let placeholder = self.composer_placeholder(id);
        self.message_menu = None;
        if self.edit.as_ref().is_some_and(|e| e.channel != id) {
            self.edit = None;
        }
        if let Some(user) = user {
            self.profile(user, None);
        }
        self.composer.open(id, placeholder, cx);
        self.autocomplete.close();
        self.suggestions.clear();

        if kind.is_forum() {
            self.open_forum(id, cx);
            self.sync_member_list(cx);
            cx.notify();
            return;
        }
        if self.history.open(id, readable, &self.messages) {
            let page = self.first_page(id);
            self.net.post(Command::History { channel: id, page });
        }
        if let Some(last) = self.newest_message(id) {
            self.ack(id, last, cx);
        }
        self.sync_member_list(cx);
        window.focus(&self.composer.input.focus_handle(cx));
        cx.notify();
    }

    /// Opens a channel named by a mention or a `discord.com/channels/…`
    /// link, switching space when it lives in another, and jumps to
    /// `message` in it. A channel this client cannot see, in a server the
    /// user is not in or hidden from them, stays closed with a note.
    pub(super) fn open_channel_link(
        &mut self,
        guild: Option<Snowflake>,
        channel: Snowflake,
        message: Option<Snowflake>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let in_dms = self.spaces.dms().channels().iter().any(|c| c.id == channel);
        let space = match self.spaces.guild_id_of(channel) {
            Some(guild) => self.spaces.index(guild),
            None if in_dms => Some(0),
            None => None,
        };
        let visible = space.is_some_and(|ix| {
            let space = self.spaces.iter().nth(ix);
            space.is_some_and(|s| s.is_dms() || s.permissions(channel) & permissions::VIEW_CHANNEL != 0)
        });
        let Some(ix) = space.filter(|_| visible) else {
            // Named for the log only by whether a guild was given, never by id.
            tracing::info!("link to an unknown channel (guild given: {})", guild.is_some());
            self.connection.set_message(t!("chat.link_unknown"));
            cx.notify();
            return;
        };
        if ix != self.spaces.selected() {
            self.spaces.select(ix);
            let (is_dms, guild) = (self.spaces.current().is_dms(), self.spaces.current().id);
            self.reveal_guild(guild);
            if !is_dms && self.spaces.subscribe(guild) {
                self.net.post(Command::SubscribeGuild { guild, members: None });
            }
        }
        // Set before opening, so a channel not loaded yet opens on the
        // page around the message instead of its newest one.
        if let Some(message) = message {
            self.start_jump(channel, JumpTarget::Message(message));
        }
        if self.selected_channel != Some(channel) || self.friends_open {
            self.select_channel(channel, window, cx);
        }
        self.continue_jump(channel, cx);
    }

    /// Opens `thread`, learning it first when it is archived or new.
    pub(super) fn open_thread(&mut self, thread: Channel, window: &mut Window, cx: &mut Context<Self>) {
        let id = thread.id;
        let current = self.spaces.current().id;
        if let Some(space) = self.spaces.get_mut(current)
            && !space.channels().iter().any(|c| c.id == id)
        {
            space.add_threads(vec![thread]);
        }
        self.select_channel(id, window, cx);
    }

    /// Enter in the DM search: opens the first match and clears the search.
    pub(super) fn open_first_dm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dms = self.spaces.dms();
        let first = dms
            .search(self.dm_search.read(cx).text())
            .into_iter()
            .find_map(|ix| match &dms.rows()[ix] {
                Row::Channel(channel) => Some(channel.id),
                _ => None,
            });
        let Some(channel) = first else { return };
        self.dm_search.update(cx, |input, cx| input.set_text("", cx));
        self.spaces.select_dms();
        self.select_channel(channel, window, cx);
    }

    /// Opens the DM with `user`, selecting it when it is already listed and
    /// asking Discord for it otherwise.
    pub(super) fn open_dm(&mut self, user: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        match self.spaces.dm_with(user) {
            Some(channel) => {
                self.spaces.select_dms();
                self.select_channel(channel, window, cx);
            }
            None => {
                self.net.post(Command::OpenDm { user, select: true });
            }
        }
    }

    /// Rows of the current space's sidebar: DMs filtered by the search,
    /// guild channels minus collapsed categories.
    pub(super) fn visible_rows(&self, cx: &App) -> Vec<usize> {
        let space = self.spaces.current();
        if space.is_dms() {
            return space.search(self.dm_search.read(cx).text());
        }
        let guild = space.id;
        let now = crate::time::now_secs();
        let pinned = |id| self.selected_channel == Some(id) || self.reads.is_unread(id) || self.reads.mentions(id) > 0;
        let rows = space.expanded(|category| self.notify.is_collapsed(guild, category), pinned);
        if !self.notify.hides_muted(guild) {
            return rows;
        }
        // "Hide Muted Channels": unless open or pinging the user.
        rows.into_iter()
            .filter(|ix| match &space.rows()[*ix] {
                Row::Channel(row) => {
                    let pinging = self.selected_channel == Some(row.id) || self.reads.mentions(row.id) > 0;
                    pinging || !self.notify.muted(&self.place_of(Some(guild), row.id), now)
                }
                _ => true,
            })
            .collect()
    }

    pub(super) fn can_send(&self, channel: Snowflake) -> bool {
        !self.history.is_forbidden(channel)
            && self.spaces.current().permissions(channel) & permissions::SEND_MESSAGES != 0
    }

    pub(super) fn can_attach(&self, channel: Snowflake) -> bool {
        self.can_send(channel) && self.spaces.current().permissions(channel) & permissions::ATTACH_FILES != 0
    }

    /// Newest known message in `channel`: the loaded log, else the channel's
    /// `last_message_id`.
    pub(super) fn newest_message(&self, channel: Snowflake) -> Option<Snowflake> {
        let listed = self.spaces.channel(channel).and_then(|c| c.last_message_id);
        self.history.newest(channel).max(listed)
    }

    /// Sends a read receipt for `message` unless it is already read. Bursts
    /// in a busy channel collapse into one receipt for the newest message.
    pub(super) fn ack(&mut self, channel: Snowflake, message: Snowflake, cx: &mut Context<Self>) {
        if !self.reads.schedule(channel, message) {
            return;
        }
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(ACK_DELAY).await;
            this.update(cx, |this, _| {
                // Only while the channel is still on screen.
                let newest = (this.selected_channel == Some(channel))
                    .then(|| this.newest_message(channel))
                    .flatten();
                if let Some(message) = this.reads.flush(channel, newest) {
                    this.net.post(Command::Ack { channel, message });
                }
            })
            .ok();
        })
        .detach();
    }

    /// Asks the gateway for the members behind these messages' authors, so
    /// names and role colors match the guild, as the web client does.
    pub(super) fn request_authors(&mut self, channel: Snowflake) {
        let Some((guild, info)) = self.spaces.guild_of(channel) else { return };
        let missing = info.unrequested(self.history.authors(channel));
        for users in missing.chunks(MEMBERS_PER_REQUEST) {
            self.net.post(Command::RequestMembers {
                guild,
                users: users.to_vec(),
            });
        }
    }

    pub(super) fn load_older(&mut self, cx: &mut Context<Self>) {
        let Some(channel) = self.selected_channel else { return };
        let Some(oldest) = self.history.older(channel) else { return };
        self.net.post(Command::History {
            channel,
            page: Page::Before(oldest),
        });
        cx.notify();
    }

    /// Requests the profile of `user` unless a recent copy is cached, like
    /// the web client; a stale copy stays on screen until the answer.
    pub(super) fn profile(&mut self, user: Snowflake, guild: Option<Snowflake>) {
        if self.profiles.request((user, guild), Instant::now()) {
            self.net.post(Command::FetchProfile { user, guild });
        }
    }
}
