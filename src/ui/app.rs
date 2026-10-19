use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

mod add_server;
mod chat;
mod friends;
mod profile;
mod settings;
mod widgets;

use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, Focusable, FontWeight, Image, ImageFormat,
    KeyBinding, ListAlignment, ListOffset, ListState, SharedString, Subscription, Window, actions,
    div, prelude::*, px, relative, rgb, uniform_list,
};
use tokio::sync::mpsc::UnboundedSender;

use super::icons::{self, icon};
use super::text_input::{Submitted, TextInput};
use super::{theme, time};
use crate::discord::model::{
    Channel, Guild, Message, Profile, Snowflake, User, avatar_url, channel_kind,
    relationship_kind,
};
use crate::discord::rest::{self, HISTORY_PAGE};
use crate::discord::{Action, Command, NetEvent};
use crate::settings::Settings;
use add_server::AddServer;
use friends::FriendsTab;
use settings::SettingsView;

actions!(emerald_ui, [Dismiss]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", Dismiss, Some("Emerald"))]);
}

/// Channels whose history stays in memory; the least recently viewed are evicted.
const CACHED_CHANNELS: usize = 8;
/// Messages kept per cached channel; older ones are dropped and refetched on demand.
const MESSAGES_PER_CHANNEL: usize = 300;
const MAX_MESSAGE_LEN: usize = 2000;
/// Pseudo-space id holding direct messages.
const DMS: Snowflake = Snowflake(0);
/// Status text the network layer reports once the session is live.
const CONNECTED: &str = "Conectado";
/// Height of the floating user panel plus its margins; lists pad by this.
const PANEL_CLEARANCE: f32 = 72.;
/// Messages from one author closer than this share a group, like Discord.
const GROUP_WINDOW_SECS: i64 = 7 * 60;
/// Rows above the messages in the chat list: the channel intro once the
/// whole history is loaded, an empty spacer before that.
const TOP: usize = 1;
/// The web client re-sends the typing signal about this often while typing.
const TYPING_INTERVAL: Duration = Duration::from_secs(8);
/// How long someone shows as typing after their last signal.
const TYPING_TIMEOUT: Duration = Duration::from_secs(10);
/// A cached profile is refetched when opened after this long.
const PROFILE_TTL: Duration = Duration::from_secs(10 * 60);
/// The profile panel only fits beside the chat in wide windows.
const PROFILE_MIN_WINDOW: f32 = 1100.;

struct Msg {
    id: Snowflake,
    author_id: Snowflake,
    author: SharedString,
    avatar_url: SharedString,
    secs: i64,
    time: SharedString,
    short_time: SharedString,
    content: SharedString,
    /// Local calendar day, for day separators.
    day: i32,
    /// Sticker names with their image URL (`None` for Lottie).
    stickers: Vec<(SharedString, Option<SharedString>)>,
    /// Set on our own sends until the server confirms them.
    nonce: Option<String>,
    /// Sent but not yet confirmed; drawn dimmed.
    pending: bool,
}

impl Msg {
    /// Our own message as typed, shown until the server echoes it.
    fn pending(me: &User, content: String, nonce: String) -> Self {
        let secs = time::now_secs();
        Self {
            id: Snowflake(0),
            author_id: me.id,
            author: me.display_name().to_owned().into(),
            avatar_url: me.avatar_url().into(),
            secs,
            time: time::header(secs).into(),
            short_time: time::short(secs).into(),
            content: content.into(),
            day: time::day_key(secs),
            stickers: Vec::new(),
            nonce: Some(nonce),
            pending: true,
        }
    }

    fn new(message: Message) -> Self {
        let mut content = message.content;
        for attachment in &message.attachments {
            if !content.is_empty() {
                content.push('\n');
            }
            content.push_str("[anexo] ");
            content.push_str(&attachment.filename);
        }
        if content.is_empty() && !message.embeds.is_empty() && message.sticker_items.is_empty() {
            content.push_str("[embed]");
        }
        let stickers = message
            .sticker_items
            .iter()
            .map(|s| (s.name.clone().into(), s.url().map(Into::into)))
            .collect();
        let secs = message.id.unix_secs();
        Self {
            id: message.id,
            author_id: message.author.id,
            author: message.author.display_name().to_owned().into(),
            avatar_url: message.author.avatar_url().into(),
            secs,
            time: time::header(secs).into(),
            short_time: time::short(secs).into(),
            content: content.into(),
            day: time::day_key(secs),
            stickers,
            nonce: message.nonce,
            pending: false,
        }
    }
}

#[derive(Default)]
struct Log {
    messages: VecDeque<Msg>,
    complete: bool,
    loading: bool,
}

enum Row {
    /// Entry point of the friends list, first row of the DM sidebar.
    Friends,
    Category(SharedString),
    Channel {
        id: Snowflake,
        icon: &'static str,
        name: SharedString,
        /// The other person in a 1:1 DM; drawn as their avatar.
        user: Option<Snowflake>,
    },
}

struct Space {
    id: Snowflake,
    name: SharedString,
    initials: SharedString,
    channels: Vec<Channel>,
    rows: Vec<Row>,
}

impl Space {
    fn guild(guild: Guild) -> Self {
        let name = guild.display_name().to_owned();
        let mut space = Self {
            id: guild.id,
            initials: initials(&name).into(),
            name: name.into(),
            channels: guild.channels,
            rows: Vec::new(),
        };
        space.rebuild();
        space
    }

    fn rebuild(&mut self) {
        self.rows.clear();
        if self.id == DMS {
            // Most recent activity first; a DM without messages counts from
            // when it was opened, which its id encodes.
            self.channels
                .sort_unstable_by_key(|c| std::cmp::Reverse(c.last_message_id.unwrap_or(c.id)));
            self.rows.push(Row::Friends);
            self.rows.push(Row::Category("MENSAGENS DIRETAS".into()));
            self.rows.extend(self.channels.iter().map(|c| Row::Channel {
                id: c.id,
                icon: "@",
                name: c.name.as_deref().unwrap_or("?").to_owned().into(),
                user: dm_recipient(c),
            }));
            return;
        }

        let readable = |c: &&Channel| {
            matches!(c.kind, channel_kind::GUILD_TEXT | channel_kind::ANNOUNCEMENT)
        };
        let order = |a: &&Channel, b: &&Channel| (a.position, a.id).cmp(&(b.position, b.id));
        let row = |c: &Channel| Row::Channel {
            id: c.id,
            icon: "#",
            name: c.name.as_deref().unwrap_or("?").to_owned().into(),
            user: None,
        };

        let mut loose: Vec<_> = self
            .channels
            .iter()
            .filter(|c| readable(c) && c.parent_id.is_none())
            .collect();
        loose.sort_unstable_by(order);
        self.rows.extend(loose.into_iter().map(row));

        let mut categories: Vec<_> = self
            .channels
            .iter()
            .filter(|c| c.kind == channel_kind::CATEGORY)
            .collect();
        categories.sort_unstable_by(order);
        for category in categories {
            let mut children: Vec<_> = self
                .channels
                .iter()
                .filter(|c| readable(c) && c.parent_id == Some(category.id))
                .collect();
            if children.is_empty() {
                continue;
            }
            children.sort_unstable_by(order);
            let title = category.name.as_deref().unwrap_or_default().to_uppercase();
            self.rows.push(Row::Category(title.into()));
            self.rows.extend(children.into_iter().map(row));
        }
    }

    fn channel(&self, id: Snowflake) -> Option<(&'static str, SharedString, Option<Snowflake>)> {
        self.rows.iter().find_map(|row| match row {
            Row::Channel {
                id: row_id,
                icon,
                name,
                user,
            } if *row_id == id => Some((*icon, name.clone(), *user)),
            _ => None,
        })
    }

    fn has_unread(&self, unread: &HashSet<Snowflake>) -> bool {
        self.channels.iter().any(|c| unread.contains(&c.id))
    }
}

/// Something drawn above the main layout; Escape dismisses it.
enum Overlay {
    AddServer(AddServer),
    Settings(SettingsView),
    /// "Ver Perfil Completo" for this user.
    Profile(Snowflake),
}

/// Result line under a form, green on success and red on failure.
struct Feedback {
    ok: bool,
    text: SharedString,
}

pub struct Emerald {
    commands: UnboundedSender<Command>,
    settings: Settings,
    me: Option<User>,
    /// Every user seen so far, for friend and DM names.
    users: HashMap<Snowflake, User>,
    /// Relationship kind per user (`relationship_kind`).
    relationships: HashMap<Snowflake, u8>,
    /// Friend presences; missing means offline.
    presences: HashMap<Snowflake, SharedString>,
    /// Avatars by CDN URL; `None` while downloading or after a failure, so
    /// each URL is requested once. Interior mutability lets render request
    /// what it is about to draw.
    avatars: RefCell<HashMap<SharedString, Option<Arc<Image>>>>,
    friends_open: bool,
    friends_tab: FriendsTab,
    friend_input: Entity<TextInput>,
    friend_feedback: Option<Feedback>,
    /// Username of the request in flight, for the success message.
    friend_pending: Option<String>,
    /// Friend whose "Remover" was clicked once and awaits confirmation.
    confirm_remove: Option<Snowflake>,
    overlay: Option<Overlay>,
    busy: HashSet<Action>,
    /// Select the next new guild, after a join or create succeeds.
    select_new_guild: bool,
    status: SharedString,
    /// Gateway session is live; drives the user panel's status dot.
    connected: bool,
    fatal: Option<SharedString>,
    spaces: Vec<Space>,
    selected_space: usize,
    selected_channel: Option<Snowflake>,
    logs: HashMap<Snowflake, Log>,
    recent: VecDeque<Snowflake>,
    unread: HashSet<Snowflake>,
    forbidden: HashSet<Snowflake>,
    messages: ListState,
    input: Entity<TextInput>,
    /// Why the last message in the open channel was not sent.
    send_error: Option<SharedString>,
    /// Filters the DM list by name; enter opens the first match.
    dm_search: Entity<TextInput>,
    /// DM profiles by user with when they were requested; `None` until the
    /// first answer arrives.
    profiles: HashMap<Snowflake, (Instant, Option<Profile>)>,
    /// Who is typing where, with when their signal expires.
    typing: HashMap<Snowflake, Vec<(Snowflake, Instant)>>,
    /// Last typing signal we sent, to throttle it.
    typing_sent: Option<(Snowflake, Instant)>,
    /// Draft text when the input last changed, to tell edits from cursor moves.
    draft: SharedString,
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl Emerald {
    pub fn new(
        commands: UnboundedSender<Command>,
        settings: Settings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| TextInput::new("Selecione um canal", cx));
        let subscription = cx.subscribe(&input, |this, _, event: &Submitted, cx| {
            this.send(event.0.clone(), cx)
        });
        let draft_subscription = cx.observe(&input, |this, _, cx| this.on_draft_changed(cx));
        let friend_input = cx.new(|cx| {
            TextInput::new("Nome de usuário", cx)
                .keep_on_submit()
        });
        let friend_subscription = cx.subscribe(&friend_input, |this, _, event: &Submitted, cx| {
            this.send_friend_request(event.0.clone(), cx)
        });

        let dm_search = cx.new(|cx| {
            TextInput::new("Encontre ou comece uma conversa", cx)
                .keep_on_submit()
                .compact()
        });
        let search_subscriptions = [
            cx.observe(&dm_search, |_, _, cx| cx.notify()),
            cx.subscribe_in(&dm_search, window, |this, _, _: &Submitted, window, cx| {
                this.open_first_dm(window, cx)
            }),
        ];

        let messages = ListState::new(0, ListAlignment::Bottom, px(400.));
        let weak = cx.entity().downgrade();
        messages.set_scroll_handler(move |event, _, cx| {
            if event.visible_range.start == 0 && event.count > 0 {
                let weak = weak.clone();
                // The list state is borrowed while this handler runs.
                cx.defer(move |cx| {
                    weak.update(cx, |this, cx| this.load_older(cx)).ok();
                });
            }
        });

        Self {
            commands,
            settings,
            me: None,
            users: HashMap::new(),
            relationships: HashMap::new(),
            presences: HashMap::new(),
            avatars: RefCell::default(),
            friends_open: true,
            friends_tab: FriendsTab::Online,
            friend_input,
            friend_feedback: None,
            friend_pending: None,
            confirm_remove: None,
            overlay: None,
            busy: HashSet::new(),
            select_new_guild: false,
            status: "Conectando…".into(),
            connected: false,
            fatal: None,
            spaces: vec![dm_space(Vec::new(), &mut HashMap::new())],
            selected_space: 0,
            selected_channel: None,
            logs: HashMap::new(),
            recent: VecDeque::new(),
            unread: HashSet::new(),
            forbidden: HashSet::new(),
            messages,
            input,
            send_error: None,
            focus_handle: cx.focus_handle(),
            dm_search,
            profiles: HashMap::new(),
            typing: HashMap::new(),
            typing_sent: None,
            draft: SharedString::default(),
            _subscriptions: {
                let mut subscriptions = vec![subscription, friend_subscription, draft_subscription];
                subscriptions.extend(search_subscriptions);
                subscriptions
            },
        }
    }

    pub fn input_focus(&self, cx: &gpui::App) -> FocusHandle {
        self.input.focus_handle(cx)
    }

    pub fn on_net(&mut self, event: NetEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            NetEvent::Status(status) => {
                self.connected = status == CONNECTED;
                self.status = status.into();
            }
            NetEvent::Fatal(reason) => self.fatal = Some(reason.into()),
            // Intercepted in `main`, which restarts into the login flow.
            NetEvent::AuthExpired | NetEvent::LoggedOut { .. } => {}
            NetEvent::Ready {
                me,
                guilds,
                dms,
                users,
                relationships,
            } => {
                self.connected = true;
                self.status = CONNECTED.into();
                self.users = users.into_iter().map(|u| (u.id, u)).collect();
                self.relationships.clear();
                for relationship in relationships {
                    self.relationships.insert(relationship.id, relationship.kind);
                    if let Some(user) = relationship.user {
                        self.users.insert(user.id, user);
                    }
                }
                self.me = Some(me);
                let previous = self.spaces.get(self.selected_space).map(|s| s.id);
                self.spaces = std::iter::once(dm_space(dms, &mut self.users))
                    .chain(guilds.into_iter().filter(|g| !g.unavailable).map(Space::guild))
                    .collect();
                self.selected_space = previous
                    .and_then(|id| self.spaces.iter().position(|s| s.id == id))
                    .unwrap_or(if self.spaces.len() > 1 { 1 } else { 0 });
                // A new session (not a resume) may have missed messages:
                // drop cached history and refetch only what is on screen.
                self.logs.clear();
                self.recent.clear();
                if let Some(channel) = self.selected_channel.filter(|_| !self.friends_open) {
                    self.select_channel(channel, window, cx);
                }
            }
            NetEvent::Presences(presences) => {
                for (user, status) in presences {
                    if status == "offline" {
                        self.presences.remove(&user);
                    } else {
                        self.presences.insert(user, status.into());
                    }
                }
            }
            NetEvent::RelationshipAdd(relationship) => {
                let id = relationship.id;
                let previous = self.relationships.insert(id, relationship.kind);
                if let Some(user) = relationship.user {
                    self.users.insert(user.id, user);
                }
                // A new friend (either side accepted) gets a DM in the list
                // right away, opened quietly in the background.
                let befriended = relationship.kind == relationship_kind::FRIEND
                    && previous != Some(relationship_kind::FRIEND);
                if befriended && self.dm_with(id).is_none() {
                    self.commands
                        .send(Command::OpenDm {
                            user: id,
                            select: false,
                        })
                        .ok();
                }
            }
            NetEvent::RelationshipRemove(user) => {
                self.relationships.remove(&user);
                if self.confirm_remove == Some(user) {
                    self.confirm_remove = None;
                }
            }
            NetEvent::UserUpdate(user) => {
                if self.me.as_ref().is_some_and(|me| me.id == user.id) {
                    // USER_UPDATE for someone else never carries the email.
                    let email = self.me.as_mut().and_then(|me| me.email.take());
                    let mut me = user.clone();
                    me.email = me.email.or(email);
                    self.me = Some(me);
                }
                self.users.insert(user.id, user);
            }
            NetEvent::DmOpened { mut channel, select } => {
                normalize_dm(&mut channel, &mut self.users);
                let id = channel.id;
                self.add_dm(channel);
                if select {
                    self.selected_space = 0;
                    self.select_channel(id, window, cx);
                }
            }
            NetEvent::Avatar { url, bytes } => {
                // Avatars are PNG; stickers may be GIF.
                let format = if url.contains(".gif") { ImageFormat::Gif } else { ImageFormat::Png };
                let image = bytes.map(|bytes| Arc::new(Image::from_bytes(format, bytes)));
                self.avatars.borrow_mut().insert(url.into(), image);
            }
            NetEvent::ActionDone(action) => self.on_action_result(action, Ok(()), window, cx),
            NetEvent::ActionFailed { action, error } => {
                self.on_action_result(action, Err(error), window, cx)
            }
            NetEvent::GuildCreate(guild) => {
                let space = Space::guild(guild);
                match self.spaces.iter_mut().find(|s| s.id == space.id) {
                    Some(existing) => *existing = space,
                    None => {
                        self.spaces.push(space);
                        if std::mem::take(&mut self.select_new_guild) {
                            self.select_space(self.spaces.len() - 1, cx);
                        }
                    }
                }
            }
            NetEvent::GuildDelete(id) => {
                if let Some(ix) = self.spaces.iter().position(|s| s.id == id) {
                    self.spaces.remove(ix);
                    if self.selected_space >= ix {
                        self.selected_space = self.selected_space.saturating_sub(1);
                    }
                }
            }
            NetEvent::ChannelCreate(mut channel) => {
                let space_id = match channel.guild_id {
                    Some(id) => id,
                    None => {
                        normalize_dm(&mut channel, &mut self.users);
                        DMS
                    }
                };
                if let Some(space) = self.spaces.iter_mut().find(|s| s.id == space_id) {
                    space.channels.retain(|c| c.id != channel.id);
                    space.channels.push(channel);
                    space.rebuild();
                }
            }
            NetEvent::ChannelDelete(channel) => {
                let space_id = channel.guild_id.unwrap_or(DMS);
                if let Some(space) = self.spaces.iter_mut().find(|s| s.id == space_id) {
                    space.channels.retain(|c| c.id != channel.id);
                    space.rebuild();
                }
                self.forget(channel.id);
            }
            NetEvent::MessageCreate(message) => {
                let channel = message.channel_id;
                let selected = self.selected_channel == Some(channel);
                let mine = self.me.as_ref().is_some_and(|me| me.id == message.author.id);
                if message.guild_id.is_none() {
                    self.touch_dm(&message);
                }
                if !selected && !mine {
                    self.unread.insert(channel);
                }
                // A message ends its author's typing indicator.
                if let Some(list) = self.typing.get_mut(&channel) {
                    list.retain(|(user, _)| *user != message.author.id);
                }
                if let Some(log) = self.logs.get_mut(&channel) {
                    if log.messages.iter().any(|m| m.id == message.id) {
                        return;
                    }
                    // Our own send coming back: confirm the pending copy in place.
                    let pending = message.nonce.as_ref().filter(|_| mine).and_then(|nonce| {
                        log.messages
                            .iter()
                            .position(|m| m.pending && m.nonce.as_ref() == Some(nonce))
                    });
                    if let Some(ix) = pending {
                        log.messages[ix] = Msg::new(message);
                        if selected {
                            self.messages.splice(TOP + ix..TOP + ix + 1, 1);
                        }
                        return;
                    }
                    let end = log.messages.len();
                    log.messages.push_back(Msg::new(message));
                    let trimmed = log.messages.len() > MESSAGES_PER_CHANNEL;
                    if trimmed {
                        log.messages.pop_front();
                        log.complete = false;
                    }
                    if selected {
                        self.messages.splice(TOP + end..TOP + end, 1);
                        if trimmed {
                            self.messages.splice(TOP..TOP + 1, 0);
                            // No longer at the start: the intro goes away.
                            self.messages.splice(0..TOP, TOP);
                        }
                    }
                }
            }
            NetEvent::MessageUpdate(update) => {
                let Some(content) = update.content else { return };
                let Some(log) = self.logs.get_mut(&update.channel_id) else { return };
                if let Some(ix) = log.messages.iter().position(|m| m.id == update.id) {
                    log.messages[ix].content = format!("{content} (editada)").into();
                    if self.selected_channel == Some(update.channel_id) {
                        self.messages.splice(TOP + ix..TOP + ix + 1, 1);
                    }
                }
            }
            NetEvent::MessageDelete(delete) => {
                let Some(log) = self.logs.get_mut(&delete.channel_id) else { return };
                if let Some(ix) = log.messages.iter().position(|m| m.id == delete.id) {
                    log.messages.remove(ix);
                    if self.selected_channel == Some(delete.channel_id) {
                        self.messages.splice(TOP + ix..TOP + ix + 1, 0);
                    }
                }
            }
            NetEvent::History {
                channel,
                before,
                messages,
            } => {
                let log = self.logs.entry(channel).or_default();
                log.loading = false;
                log.complete = messages.len() < HISTORY_PAGE;
                let count = messages.len();
                let selected = self.selected_channel == Some(channel);
                match before {
                    None => {
                        log.messages = messages.into_iter().map(Msg::new).collect();
                        if selected {
                            self.messages.reset(TOP + log.messages.len());
                        }
                    }
                    Some(_) => {
                        let room = MESSAGES_PER_CHANNEL.saturating_sub(log.messages.len());
                        let skip = count.saturating_sub(room);
                        let added = count - skip;
                        if added == 0 {
                            log.complete = true;
                        }
                        for message in messages.into_iter().skip(skip).rev() {
                            log.messages.push_front(Msg::new(message));
                        }
                        if selected {
                            let top = self.messages.logical_scroll_top();
                            self.messages.splice(TOP..TOP, added);
                            if log.complete {
                                // The top row turns into the channel intro.
                                self.messages.splice(0..TOP, TOP);
                            }
                            if added > 0 {
                                self.messages.scroll_to(ListOffset {
                                    item_ix: top.item_ix + added,
                                    offset_in_item: top.offset_in_item,
                                });
                            }
                        }
                    }
                }
            }
            NetEvent::HistoryFailed {
                channel,
                forbidden,
                error,
            } => {
                if let Some(log) = self.logs.get_mut(&channel) {
                    log.loading = false;
                }
                if forbidden {
                    self.forbidden.insert(channel);
                    self.forget(channel);
                }
                self.status = format!("Erro: {error}").into();
            }
            NetEvent::TypingStart(typing) => {
                if self.me.as_ref().is_some_and(|me| me.id == typing.user_id) {
                    return;
                }
                let until = Instant::now() + TYPING_TIMEOUT;
                let list = self.typing.entry(typing.channel_id).or_default();
                list.retain(|(user, _)| *user != typing.user_id);
                list.push((typing.user_id, until));
                // Redraw when it expires so the indicator goes away on its own.
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(TYPING_TIMEOUT).await;
                    this.update(cx, |this, cx| {
                        let now = Instant::now();
                        this.typing.retain(|_, list| {
                            list.retain(|(_, until)| *until > now);
                            !list.is_empty()
                        });
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            }
            NetEvent::Profile(profile) => {
                let slot = self
                    .profiles
                    .entry(profile.user.id)
                    .or_insert_with(|| (Instant::now(), None));
                slot.1 = Some(profile);
            }
            NetEvent::SendFailed {
                channel,
                nonce,
                content,
                error,
            } => {
                if let Some(log) = self.logs.get_mut(&channel) {
                    if let Some(ix) = log
                        .messages
                        .iter()
                        .position(|m| m.pending && m.nonce.as_ref() == Some(&nonce))
                    {
                        log.messages.remove(ix);
                        if self.selected_channel == Some(channel) {
                            self.messages.splice(TOP + ix..TOP + ix + 1, 0);
                        }
                    }
                }
                if self.selected_channel == Some(channel) {
                    self.send_error = Some(error.into());
                    // Give the text back unless the user already typed more.
                    self.input.update(cx, |input, cx| {
                        if input.text().is_empty() {
                            input.set_text(content, cx);
                        }
                    });
                }
            }
        }
    }

    /// The avatar at `url` if downloaded; otherwise starts the download (once).
    fn avatar_image(&self, url: &str) -> Option<Arc<Image>> {
        let mut avatars = self.avatars.borrow_mut();
        if let Some(slot) = avatars.get(url) {
            return slot.clone();
        }
        avatars.insert(url.to_owned().into(), None);
        self.commands
            .send(Command::FetchAvatar { url: url.to_owned() })
            .ok();
        None
    }

    /// Avatar of a known user, or the default one their id maps to.
    fn user_avatar(&self, id: Snowflake) -> Option<Arc<Image>> {
        let url = match self.users.get(&id) {
            Some(user) => user.avatar_url(),
            None => avatar_url(id, None),
        };
        self.avatar_image(&url)
    }

    /// The open 1:1 DM with `user`, if any.
    fn dm_with(&self, user: Snowflake) -> Option<Snowflake> {
        let space = self.spaces.iter().find(|s| s.id == DMS)?;
        space
            .channels
            .iter()
            .find(|c| c.kind == channel_kind::DM && dm_recipient(c) == Some(user))
            .map(|c| c.id)
    }

    /// Rows of the DM sidebar matching the search, as indices into its rows.
    /// While searching only matching conversations are listed.
    fn visible_dm_rows(&self, space: &Space, cx: &App) -> Vec<usize> {
        let query = self.dm_search.read(cx).text().trim().to_lowercase();
        if query.is_empty() {
            return (0..space.rows.len()).collect();
        }
        space
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| match row {
                Row::Channel { name, .. } => name.to_lowercase().contains(&query),
                _ => false,
            })
            .map(|(ix, _)| ix)
            .collect()
    }

    /// Enter in the DM search: opens the first match and clears the search.
    fn open_first_dm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(space) = self.spaces.iter().position(|s| s.id == DMS) else { return };
        let first = self
            .visible_dm_rows(&self.spaces[space], cx)
            .into_iter()
            .find_map(|ix| match self.spaces[space].rows[ix] {
                Row::Channel { id, .. } => Some(id),
                _ => None,
            });
        let Some(channel) = first else { return };
        self.dm_search.update(cx, |input, cx| input.set_text("", cx));
        self.selected_space = space;
        self.select_channel(channel, window, cx);
    }

    /// Inserts or replaces a DM in the sidebar.
    fn add_dm(&mut self, channel: Channel) {
        if let Some(space) = self.spaces.iter_mut().find(|s| s.id == DMS) {
            space.channels.retain(|c| c.id != channel.id);
            space.channels.push(channel);
            space.rebuild();
        }
    }

    /// Moves the DM a message landed in to the top of the sidebar, adding
    /// it first when the conversation was closed or never opened here.
    fn touch_dm(&mut self, message: &Message) {
        let Some(space) = self.spaces.iter_mut().find(|s| s.id == DMS) else { return };
        if let Some(channel) = space.channels.iter_mut().find(|c| c.id == message.channel_id) {
            channel.last_message_id = Some(message.id);
            space.rebuild();
            return;
        }
        let mine = self.me.as_ref().is_some_and(|me| me.id == message.author.id);
        let mut channel = Channel {
            id: message.channel_id,
            kind: channel_kind::DM,
            name: None,
            position: 0,
            parent_id: None,
            guild_id: None,
            last_message_id: Some(message.id),
            // From someone else, the author is the other side of the DM.
            recipient_ids: if mine { Vec::new() } else { vec![message.author.id] },
            recipients: Vec::new(),
        };
        if !mine {
            self.users.insert(message.author.id, message.author.clone());
        }
        normalize_dm(&mut channel, &mut self.users);
        self.add_dm(channel);
    }

    /// Sends the typing signal while the user edits a draft, at most every
    /// `TYPING_INTERVAL` per channel.
    fn on_draft_changed(&mut self, cx: &mut Context<Self>) {
        let text = self.input.read(cx).text();
        if text == self.draft.as_ref() {
            return;
        }
        self.draft = text.to_owned().into();
        let Some(channel) = self.selected_channel else { return };
        if self.draft.trim().is_empty() || self.forbidden.contains(&channel) {
            return;
        }
        let recent = self
            .typing_sent
            .is_some_and(|(sent_in, at)| sent_in == channel && at.elapsed() < TYPING_INTERVAL);
        if !recent {
            self.typing_sent = Some((channel, Instant::now()));
            self.commands.send(Command::Typing { channel }).ok();
        }
    }

    /// Requests the profile of `user` unless a recent copy is cached, like
    /// the web client; a stale copy stays on screen until the answer.
    fn profile(&mut self, user: Snowflake) {
        let now = Instant::now();
        match self.profiles.get_mut(&user) {
            Some((requested, _)) if requested.elapsed() < PROFILE_TTL => return,
            Some((requested, _)) => *requested = now,
            None => {
                self.profiles.insert(user, (now, None));
            }
        }
        self.commands.send(Command::FetchProfile { user }).ok();
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
                let username = self.friend_pending.take().unwrap_or_default();
                self.friend_feedback = Some(match result {
                    Ok(()) => {
                        self.friend_input.update(cx, |input, cx| input.set_text("", cx));
                        Feedback {
                            ok: true,
                            text: format!("Sucesso! Seu pedido de amizade para {username} foi enviado.")
                                .into(),
                        }
                    }
                    Err(error) => Feedback {
                        ok: false,
                        text: error.into(),
                    },
                });
            }
            Action::Relationship => {
                if let Err(error) = result {
                    self.status = format!("Erro: {error}").into();
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
                    view.feedback = Some(match result {
                        Ok(()) => Feedback {
                            ok: true,
                            text: "Nome exibido atualizado.".into(),
                        },
                        Err(error) => Feedback {
                            ok: false,
                            text: error.into(),
                        },
                    });
                }
            }
        }
    }

    /// Sends a REST action unless the same kind is already in flight.
    fn run(&mut self, action: Action, command: Command, cx: &mut Context<Self>) -> bool {
        if !self.busy.insert(action) {
            return false;
        }
        self.commands.send(command).ok();
        cx.notify();
        true
    }

    fn close_overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.overlay.take().is_some() {
            window.focus(&self.input.focus_handle(cx));
            cx.notify();
        }
    }

    /// Asks the network thread to revoke the token; `main` signs out locally
    /// when the answer arrives. Without a network thread, signs out at once.
    fn log_out(&mut self, cx: &mut Context<Self>) {
        self.status = "Saindo…".into();
        self.connected = false;
        cx.notify();
        if self.commands.send(Command::LogOut).is_err() {
            crate::sign_out();
        }
    }

    fn save_settings(&mut self, cx: &mut Context<Self>) {
        if let Err(e) = self.settings.save() {
            log::error!("could not save settings: {e}");
        }
        cx.notify();
    }

    fn select_space(&mut self, ix: usize, cx: &mut Context<Self>) {
        self.selected_space = ix;
        if self.spaces[ix].id == DMS {
            self.friends_open = true;
        }
        cx.notify();
    }

    fn select_channel(&mut self, id: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        self.friends_open = false;
        self.selected_channel = Some(id);
        self.unread.remove(&id);
        self.recent.retain(|c| *c != id);
        self.recent.push_back(id);
        while self.recent.len() > CACHED_CHANNELS {
            if let Some(evicted) = self.recent.pop_front() {
                self.logs.remove(&evicted);
            }
        }

        self.send_error = None;
        let (icon, name, user) = self.spaces[self.selected_space]
            .channel(id)
            .unwrap_or(("#", SharedString::default(), None));
        if let Some(user) = user {
            self.profile(user);
        }
        // Same wording as the pt-BR web client.
        let placeholder = if self.forbidden.contains(&id) {
            "Sem permissão para enviar mensagens neste canal".to_owned()
        } else if icon == "@" {
            format!("Conversar com @{name}")
        } else {
            format!("Conversar em #{name}")
        };
        self.input
            .update(cx, |input, cx| input.set_placeholder(placeholder, cx));

        match self.logs.get(&id) {
            Some(log) => self.messages.reset(TOP + log.messages.len()),
            None => {
                self.messages.reset(TOP);
                if !self.forbidden.contains(&id) {
                    self.logs.insert(
                        id,
                        Log {
                            loading: true,
                            ..Log::default()
                        },
                    );
                    self.commands
                        .send(Command::History {
                            channel: id,
                            before: None,
                        })
                        .ok();
                }
            }
        }
        window.focus(&self.input.focus_handle(cx));
        cx.notify();
    }

    fn load_older(&mut self, cx: &mut Context<Self>) {
        let Some(channel) = self.selected_channel else { return };
        let Some(log) = self.logs.get_mut(&channel) else { return };
        if log.loading || log.complete || log.messages.len() >= MESSAGES_PER_CHANNEL {
            return;
        }
        let Some(oldest) = log.messages.front().map(|m| m.id) else { return };
        log.loading = true;
        self.commands
            .send(Command::History {
                channel,
                before: Some(oldest),
            })
            .ok();
        cx.notify();
    }

    fn send(&mut self, content: String, cx: &mut Context<Self>) {
        let Some(channel) = self.selected_channel else { return };
        if self.forbidden.contains(&channel) {
            return;
        }
        let content = content.trim_end().to_owned();
        if content.trim().is_empty() {
            return;
        }
        if content.chars().count() > MAX_MESSAGE_LEN {
            self.send_error =
                Some(format!("Sua mensagem passou do limite de {MAX_MESSAGE_LEN} caracteres.").into());
            self.input.update(cx, |input, cx| input.set_text(content, cx));
            cx.notify();
            return;
        }
        self.send_error = None;
        self.typing_sent = None;
        let nonce = rest::nonce();
        if let (Some(me), Some(log)) = (&self.me, self.logs.get_mut(&channel)) {
            let end = log.messages.len();
            log.messages
                .push_back(Msg::pending(me, content.clone(), nonce.clone()));
            self.messages.splice(TOP + end..TOP + end, 1);
        }
        self.commands
            .send(Command::Send {
                channel,
                content,
                nonce,
            })
            .ok();
        cx.notify();
    }

    fn forget(&mut self, channel: Snowflake) {
        self.logs.remove(&channel);
        self.recent.retain(|c| *c != channel);
        if self.selected_channel == Some(channel) {
            self.messages.reset(TOP);
        }
    }

    fn render_servers(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut items = Vec::with_capacity(self.spaces.len() + 1);
        for (ix, space) in self.spaces.iter().enumerate() {
            items.push(self.render_space(ix, space, cx).into_any_element());
            if space.id == DMS {
                // Divider between direct messages and servers.
                items.push(
                    div()
                        .flex_none()
                        .w(px(32.))
                        .h(px(1.))
                        .bg(theme::selected())
                        .into_any_element(),
                );
            }
        }

        div()
            .id("servers")
            .flex()
            .flex_col()
            .items_center()
            .gap_2()
            .pt_1()
            .pb(px(PANEL_CLEARANCE))
            .w(px(72.))
            .h_full()
            .flex_none()
            .overflow_y_scroll()
            .children(items)
            .child(
                div()
                    .id("add-server")
                    .flex_none()
                    .size(px(40.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(12.))
                    .bg(rgb(theme::BASE_LOW))
                    .text_color(rgb(theme::ONLINE))
                    .text_size(px(24.))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::ONLINE)).text_color(rgb(theme::TEXT_STRONG)))
                    .child("+")
                    .on_click(cx.listener(|this, _, window, cx| this.open_add_server(window, cx))),
            )
    }

    fn render_space(&self, ix: usize, space: &Space, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = ix == self.selected_space;
        let unread = !selected && space.has_unread(&self.unread);
        let group: SharedString = format!("space-{ix}").into();
        // White pill on the left edge: full height when selected, a dot when
        // unread, half height on hover.
        let (pill_height, pill_top) = match (selected, unread) {
            (true, _) => (40., 0.),
            (false, true) => (8., 16.),
            (false, false) => (0., 20.),
        };
        let pill = div()
            .absolute()
            .left_0()
            .top(px(pill_top))
            .w(px(4.))
            .h(px(pill_height))
            .rounded_r(px(4.))
            .bg(rgb(theme::TEXT_STRONG))
            .when(!selected, |pill| {
                pill.group_hover(group.clone(), |s| s.top(px(10.)).h(px(20.)))
            });
        let initials_size = if space.initials.chars().count() > 2 { 12. } else { 14. };

        div()
            .group(group.clone())
            .relative()
            .flex_none()
            .w_full()
            .h(px(40.))
            .flex()
            .justify_center()
            .child(pill)
            .child(
                div()
                    .id(("space", ix))
                    .size(px(40.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(12.))
                    .bg(rgb(if selected { theme::BRAND } else { theme::BASE_LOW }))
                    .text_color(rgb(if selected { theme::TEXT_STRONG } else { theme::INTERACTIVE }))
                    .text_size(px(initials_size))
                    .font_weight(FontWeight::MEDIUM)
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::BRAND)).text_color(rgb(theme::TEXT_STRONG)))
                    .child(space.initials.clone())
                    .on_click(cx.listener(move |this, _, _, cx| this.select_space(ix, cx))),
            )
    }

    fn render_channels(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let space = &self.spaces[self.selected_space];
        let dms = space.id == DMS;
        let header = div()
            .h(px(48.))
            .flex_none()
            .flex()
            .items_center()
            .border_b_1()
            .border_color(theme::border());
        let header = if dms {
            // Search field in place of a title, like the web client.
            header.px_2().child(self.dm_search.clone())
        } else {
            header
                .px_4()
                .text_color(rgb(theme::TEXT_STRONG))
                .font_weight(FontWeight::SEMIBOLD)
                .child(div().truncate().child(space.name.clone()))
        };
        let visible = if dms {
            self.visible_dm_rows(space, cx)
        } else {
            (0..space.rows.len()).collect()
        };
        let searching = dms && visible.is_empty() && !self.dm_search.read(cx).text().trim().is_empty();

        div()
            .flex()
            .flex_col()
            .w(px(240.))
            .h_full()
            .flex_none()
            .child(header)
            .when(searching, |sidebar| {
                sidebar.child(
                    div()
                        .p_4()
                        .text_size(px(14.))
                        .text_color(theme::muted())
                        .child("Nenhuma conversa encontrada."),
                )
            })
            .child(
                uniform_list(
                    "channels",
                    visible.len(),
                    cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                        range
                            .map(|ix| this.render_channel_row(visible[ix], cx))
                            .collect::<Vec<_>>()
                    }),
                )
                .flex_1()
                .pt_2()
                .pb(px(PANEL_CLEARANCE)),
            )
    }

    /// One sidebar row. The outer box spans the list width so hover and
    /// selection backgrounds fill the row, not just its text.
    fn render_channel_row(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        // uniform_list needs one height per list: DM rows fit a 32px avatar.
        let height = if self.spaces[self.selected_space].id == DMS { 42. } else { 34. };
        div()
            .w_full()
            .h(px(height))
            .px_2()
            .py(px(1.))
            .child(self.render_channel_row_body(ix, cx))
            .into_any_element()
    }

    fn render_channel_row_body(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let row = div().id(ix).size_full().px_2().flex().items_center();
        match &self.spaces[self.selected_space].rows[ix] {
            Row::Friends => {
                let selected = self.friends_open;
                let pending = self.incoming_requests();
                let color = if selected { theme::TEXT_STRONG } else { theme::CHANNELS_DEFAULT };
                row.group("friends-row")
                    .gap_3()
                    .rounded_lg()
                    .cursor_pointer()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(if selected { theme::TEXT_STRONG } else { theme::CHANNELS_DEFAULT }))
                    .when(selected, |r| r.bg(theme::selected()))
                    .when(!selected, |r| {
                        r.hover(|s| s.bg(theme::hover()).text_color(rgb(theme::TEXT_STRONG)))
                    })
                    // Icon in a 32px slot so it lines up with the DM avatars.
                    .child(
                        div()
                            .flex_none()
                            .w(px(32.))
                            .flex()
                            .justify_center()
                            .child(icon(icons::FRIENDS, 24., color).group_hover("friends-row", |s| {
                                s.text_color(rgb(theme::TEXT_STRONG))
                            })),
                    )
                    .child(div().flex_1().child("Amigos"))
                    .when(pending > 0, |r| r.child(widgets::badge(pending)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.friends_open = true;
                        cx.notify();
                    }))
                    .into_any_element()
            }
            Row::Category(title) => row
                .items_end()
                .pb(px(4.))
                .text_size(px(12.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(theme::CHANNELS_DEFAULT))
                .hover(|s| s.text_color(rgb(theme::TEXT_DEFAULT)))
                .child(div().truncate().child(title.clone()))
                .into_any_element(),
            Row::Channel { id, icon, name, user } => {
                let id = *id;
                let user = *user;
                let selected = self.selected_channel == Some(id) && !self.friends_open;
                let unread = self.unread.contains(&id);
                let forbidden = self.forbidden.contains(&id);
                let (color, icon_color) = if forbidden {
                    (theme::INTERACTIVE_MUTED, theme::INTERACTIVE_MUTED)
                } else if selected || unread {
                    (theme::TEXT_STRONG, theme::CHANNELS_DEFAULT)
                } else {
                    (theme::CHANNELS_DEFAULT, theme::CHANNELS_DEFAULT)
                };
                row.relative()
                    .gap(px(6.))
                    .rounded_lg()
                    .cursor_pointer()
                    .font_weight(if unread { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
                    .text_color(rgb(color))
                    .when(selected, |r| r.bg(theme::selected()))
                    .when(!selected && !forbidden, |r| {
                        r.hover(|s| s.bg(theme::hover()).text_color(rgb(theme::TEXT_STRONG)))
                    })
                    .when(unread && !selected, |r| {
                        // Unread marker hugging the sidebar's left edge.
                        r.child(
                            div()
                                .absolute()
                                .left(px(-8.))
                                .top(px(13.))
                                .w(px(4.))
                                .h(px(8.))
                                .rounded_r(px(4.))
                                .bg(rgb(theme::TEXT_STRONG)),
                        )
                    })
                    .map(|r| match user {
                        Some(user) => r.gap_3().child(widgets::avatar(
                            name,
                            user,
                            self.user_avatar(user),
                            32.,
                            Some((
                                self.presences.get(&user).map_or("offline", |s| s.as_ref()),
                                theme::BASE_LOWEST,
                            )),
                        )),
                        None => r.child(
                            div()
                                .flex_none()
                                .w(px(20.))
                                .flex()
                                .justify_center()
                                .text_size(px(20.))
                                .font_weight(FontWeight::NORMAL)
                                .text_color(rgb(icon_color))
                                .child(*icon),
                        ),
                    })
                    .child(div().flex_1().min_w(px(0.)).truncate().child(name.clone()))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.select_channel(id, window, cx)
                    }))
                    .into_any_element()
            }
        }
    }

    /// Status shown on the user's own avatar: the picked one, else online
    /// while connected.
    fn own_status(&self) -> Option<&'static str> {
        if !self.connected || self.fatal.is_some() {
            return None;
        }
        Some(self.settings.status.map_or("online", |s| s.as_str()))
    }

    fn render_user_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let me = self.me.as_ref();
        let name: SharedString = me.map_or("—".into(), |me| me.display_name().to_owned().into());
        let status = self.own_status();
        let (subtitle, subtitle_color) = match &self.fatal {
            Some(fatal) => (fatal.clone(), theme::DANGER),
            None if status.is_some() => (widgets::status_label(status).into(), theme::TEXT_MUTED),
            None => (self.status.clone(), theme::TEXT_MUTED),
        };

        div()
            .absolute()
            .left_2()
            .bottom_2()
            .w(px(72. + 240. - 16.))
            .h(px(56.))
            .px_2()
            .flex()
            .items_center()
            .gap_1()
            .rounded_lg()
            .border_1()
            .border_color(theme::border())
            .bg(rgb(theme::BASE_LOW))
            .child(
                // Avatar and name open the status picker, like Discord's popout.
                div()
                    .id("me")
                    .flex()
                    .flex_1()
                    .min_w(px(0.))
                    .items_center()
                    .gap_2()
                    .p_1()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme::hover()))
                    .child(widgets::avatar(
                        &name,
                        me.map_or(Snowflake(0), |me| me.id),
                        me.and_then(|me| self.avatar_image(&me.avatar_url())),
                        32.,
                        Some((status.unwrap_or("offline"), theme::BASE_LOW)),
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(px(0.))
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(theme::TEXT_DEFAULT))
                                    .child(name.clone()),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_xs()
                                    .text_color(rgb(subtitle_color))
                                    .child(subtitle),
                            ),
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_settings(settings::Page::Status, window, cx)
                    })),
            )
            .child(
                widgets::icon_button("open-settings", "⚙")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_settings(settings::Page::Account, window, cx)
                    })),
            )
    }
}

impl Focusable for Emerald {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Emerald {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let wide = f32::from(window.viewport_size().width) >= PROFILE_MIN_WINDOW;
        // Visual-refresh frame: server list on the app background, then one
        // bordered panel with a rounded top-left corner holding sidebar and
        // chat, and the user panel floating over the bottom-left corner.
        let overlay = match &self.overlay {
            Some(Overlay::AddServer(modal)) => Some(self.render_add_server(modal, cx).into_any_element()),
            Some(Overlay::Settings(view)) => Some(self.render_settings(view, cx).into_any_element()),
            Some(Overlay::Profile(user)) => Some(self.render_profile_modal(*user, cx).into_any_element()),
            None => None,
        };
        div()
            .track_focus(&self.focus_handle)
            .key_context("Emerald")
            .on_action(cx.listener(|this, _: &Dismiss, window, cx| this.close_overlay(window, cx)))
            .on_action(cx.listener(|this, _: &crate::Logout, _, cx| this.log_out(cx)))
            .relative()
            .flex()
            .size_full()
            .bg(rgb(theme::BASE_LOWEST))
            .text_size(px(16.))
            // gpui defaults to 1.618; Discord's interface sits near 1.25.
            .line_height(relative(1.25))
            .text_color(theme::text())
            .child(self.render_servers(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .rounded_tl(px(8.))
                    .border_l_1()
                    .border_t_1()
                    .border_color(theme::border())
                    .child(self.render_channels(cx))
                    .child(self.render_chat(wide, cx)),
            )
            .child(self.render_user_panel(cx))
            .children(overlay)
    }
}

fn dm_space(mut channels: Vec<Channel>, users: &mut HashMap<Snowflake, User>) -> Space {
    channels.retain(|c| matches!(c.kind, channel_kind::DM | channel_kind::GROUP_DM));
    for channel in &mut channels {
        normalize_dm(channel, users);
    }
    let mut space = Space {
        id: DMS,
        name: "Mensagens diretas".into(),
        initials: "DM".into(),
        channels,
        rows: Vec::new(),
    };
    space.rebuild();
    space
}

/// Moves a DM's embedded recipients into `users`, keeping only their ids,
/// and names the channel after them.
fn normalize_dm(channel: &mut Channel, users: &mut HashMap<Snowflake, User>) {
    if !channel.recipients.is_empty() {
        channel.recipient_ids = channel.recipients.iter().map(|u| u.id).collect();
        for user in channel.recipients.drain(..) {
            users.insert(user.id, user);
        }
    }
    channel.name = Some(dm_name(channel, users));
}

/// The other person in a 1:1 DM.
fn dm_recipient(channel: &Channel) -> Option<Snowflake> {
    match channel.kind {
        channel_kind::DM => channel
            .recipient_ids
            .first()
            .copied()
            .or(channel.recipients.first().map(|u| u.id)),
        _ => None,
    }
}

fn dm_name(channel: &Channel, users: &HashMap<Snowflake, User>) -> String {
    if let Some(name) = channel.name.as_deref().filter(|n| !n.is_empty()) {
        return name.to_owned();
    }
    let names: Vec<&str> = if channel.recipients.is_empty() {
        channel
            .recipient_ids
            .iter()
            .filter_map(|id| users.get(id).map(User::display_name))
            .collect()
    } else {
        channel.recipients.iter().map(User::display_name).collect()
    };
    if names.is_empty() { "?".into() } else { names.join(", ") }
}

fn initials(name: &str) -> String {
    let initials: String = name
        .split_whitespace()
        .filter_map(|word| word.chars().next())
        .take(3)
        .collect();
    if initials.is_empty() { "?".into() } else { initials }
}
