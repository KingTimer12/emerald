//! Moving around: picking a space or channel, paging history back, and the
//! requests that follow what is on screen (read receipts, members, profiles).

use std::time::{Duration, Instant};

use gpui::{App, Context, Focusable, SharedString, Window};

use super::Emerald;
use super::state::Row;
use crate::discord::Command;
use crate::discord::model::Snowflake;
use crate::discord::permissions;

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
        if self.spaces.subscribe(guild) {
            self.net.post(Command::SubscribeGuild { guild, members: None });
        }
        match self.spaces.landing_channel() {
            Some(channel) => self.select_channel(channel, window, cx),
            None => {
                self.selected_channel = None;
                self.members = None;
                cx.notify();
            }
        }
    }

    pub(super) fn select_channel(&mut self, id: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        self.friends_open = false;
        self.selected_channel = Some(id);
        self.spaces.remember_channel(id);
        self.reads.seen(id);

        let space = self.spaces.current();
        let readable = space.permissions(id) & permissions::READ_MESSAGE_HISTORY != 0;
        let row = space.row(id);
        let name = row.map_or_else(SharedString::default, |row| row.name.clone());
        let user = row.and_then(|row| row.user);
        // Same wording as the pt-BR web client.
        let placeholder = if !self.can_send(id) {
            "Você não tem permissão para enviar mensagens neste canal.".to_owned()
        } else if space.is_dms() {
            format!("Conversar com @{name}")
        } else {
            format!("Conversar em #{name}")
        };
        if let Some(user) = user {
            self.profile(user, None);
        }
        self.composer.open(placeholder, cx);

        if self.history.open(id, readable, &self.messages) {
            self.net.post(Command::History {
                channel: id,
                before: None,
            });
        }
        if let Some(last) = self.newest_message(id) {
            self.ack(id, last, cx);
        }
        self.sync_member_list(cx);
        window.focus(&self.composer.input.focus_handle(cx));
        cx.notify();
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
        space.expanded(self.spaces.collapsed(), |id| {
            self.selected_channel == Some(id) || self.reads.is_unread(id)
        })
    }

    pub(super) fn can_send(&self, channel: Snowflake) -> bool {
        !self.history.is_forbidden(channel)
            && self.spaces.current().permissions(channel) & permissions::SEND_MESSAGES != 0
    }

    /// Newest known message in `channel`: the loaded log, else the channel's
    /// `last_message_id`.
    fn newest_message(&self, channel: Snowflake) -> Option<Snowflake> {
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
            before: Some(oldest),
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
