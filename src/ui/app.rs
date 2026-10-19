use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

mod add_server;
mod friends;
mod settings;
mod widgets;

use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, Focusable, FontWeight, Image, ImageFormat,
    KeyBinding, ListAlignment, ListOffset, ListState, SharedString, Subscription, Window, actions,
    div, list, prelude::*, px, relative, rgb, uniform_list,
};
use tokio::sync::mpsc::UnboundedSender;

use super::text_input::{Submitted, TextInput};
use super::theme;
use crate::discord::model::{Channel, Guild, Message, Snowflake, User, avatar_url, channel_kind};
use crate::discord::rest::HISTORY_PAGE;
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

struct Msg {
    id: Snowflake,
    author_id: Snowflake,
    author: SharedString,
    avatar_url: SharedString,
    time: SharedString,
    content: SharedString,
}

impl Msg {
    fn new(message: Message) -> Self {
        let mut content = message.content;
        for attachment in &message.attachments {
            if !content.is_empty() {
                content.push('\n');
            }
            content.push_str("[anexo] ");
            content.push_str(&attachment.filename);
        }
        if content.is_empty() && !message.embeds.is_empty() {
            content.push_str("[embed]");
        }
        Self {
            id: message.id,
            author_id: message.author.id,
            author: message.author.display_name().to_owned().into(),
            avatar_url: message.author.avatar_url().into(),
            // ISO-8601 in UTC: keep "YYYY-MM-DD HH:MM".
            time: message
                .timestamp
                .get(..16)
                .map(|t| t.replace('T', " "))
                .unwrap_or_default()
                .into(),
            content: content.into(),
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
            self.channels
                .sort_unstable_by_key(|c| std::cmp::Reverse(c.last_message_id));
            self.rows.push(Row::Friends);
            self.rows.push(Row::Category("MENSAGENS DIRETAS".into()));
            self.rows.extend(self.channels.iter().map(|c| Row::Channel {
                id: c.id,
                icon: "@",
                name: c.name.as_deref().unwrap_or("?").to_owned().into(),
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

    fn channel(&self, id: Snowflake) -> Option<(&'static str, SharedString)> {
        self.rows.iter().find_map(|row| match row {
            Row::Channel {
                id: row_id,
                icon,
                name,
            } if *row_id == id => Some((*icon, name.clone())),
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
    focus_handle: FocusHandle,
    _subscriptions: [Subscription; 2],
}

impl Emerald {
    pub fn new(
        commands: UnboundedSender<Command>,
        settings: Settings,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| TextInput::new("Selecione um canal", cx));
        let subscription = cx.subscribe(&input, |this, _, event: &Submitted, cx| {
            this.send(event.0.clone(), cx)
        });
        let friend_input = cx.new(|cx| {
            TextInput::new("Nome de usuário", cx)
                .keep_on_submit()
        });
        let friend_subscription = cx.subscribe(&friend_input, |this, _, event: &Submitted, cx| {
            this.send_friend_request(event.0.clone(), cx)
        });

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
            spaces: vec![dm_space(Vec::new(), &HashMap::new())],
            selected_space: 0,
            selected_channel: None,
            logs: HashMap::new(),
            recent: VecDeque::new(),
            unread: HashSet::new(),
            forbidden: HashSet::new(),
            messages,
            input,
            focus_handle: cx.focus_handle(),
            _subscriptions: [subscription, friend_subscription],
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
            NetEvent::AuthExpired => {}
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
                self.spaces = std::iter::once(dm_space(dms, &self.users))
                    .chain(guilds.into_iter().filter(|g| !g.unavailable).map(Space::guild))
                    .collect();
                self.selected_space = previous
                    .and_then(|id| self.spaces.iter().position(|s| s.id == id))
                    .unwrap_or(if self.spaces.len() > 1 { 1 } else { 0 });
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
                self.relationships.insert(relationship.id, relationship.kind);
                if let Some(user) = relationship.user {
                    self.users.insert(user.id, user);
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
            NetEvent::DmOpened(mut channel) => {
                channel.name = Some(dm_name(&channel, &self.users));
                let id = channel.id;
                if let Some(space) = self.spaces.iter_mut().find(|s| s.id == DMS) {
                    space.channels.retain(|c| c.id != id);
                    space.channels.insert(0, channel);
                    space.rebuild();
                }
                self.selected_space = 0;
                self.select_channel(id, window, cx);
            }
            NetEvent::Avatar { url, bytes } => {
                let image = bytes.map(|bytes| Arc::new(Image::from_bytes(ImageFormat::Png, bytes)));
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
                        channel.name = Some(dm_name(&channel, &self.users));
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
                if !selected {
                    self.unread.insert(channel);
                }
                if let Some(log) = self.logs.get_mut(&channel) {
                    if log.messages.iter().any(|m| m.id == message.id) {
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
                        self.messages.splice(end..end, 1);
                        if trimmed {
                            self.messages.splice(0..1, 0);
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
                        self.messages.splice(ix..ix + 1, 1);
                    }
                }
            }
            NetEvent::MessageDelete(delete) => {
                let Some(log) = self.logs.get_mut(&delete.channel_id) else { return };
                if let Some(ix) = log.messages.iter().position(|m| m.id == delete.id) {
                    log.messages.remove(ix);
                    if self.selected_channel == Some(delete.channel_id) {
                        self.messages.splice(ix..ix + 1, 0);
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
                            self.messages.reset(log.messages.len());
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
                        if selected && added > 0 {
                            let top = self.messages.logical_scroll_top();
                            self.messages.splice(0..0, added);
                            self.messages.scroll_to(ListOffset {
                                item_ix: top.item_ix + added,
                                offset_in_item: top.offset_in_item,
                            });
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
            NetEvent::SendFailed { error } => {
                self.status = format!("Falha ao enviar: {error}").into();
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

        let (icon, name) = self.spaces[self.selected_space]
            .channel(id)
            .unwrap_or(("#", SharedString::default()));
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
            Some(log) => self.messages.reset(log.messages.len()),
            None => {
                self.messages.reset(0);
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
        if content.chars().count() > MAX_MESSAGE_LEN {
            self.status = format!("Mensagem acima de {MAX_MESSAGE_LEN} caracteres").into();
            cx.notify();
            return;
        }
        self.commands.send(Command::Send { channel, content }).ok();
    }

    fn forget(&mut self, channel: Snowflake) {
        self.logs.remove(&channel);
        self.recent.retain(|c| *c != channel);
        if self.selected_channel == Some(channel) {
            self.messages.reset(0);
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
        div()
            .flex()
            .flex_col()
            .w(px(240.))
            .h_full()
            .flex_none()
            .child(
                div()
                    .h(px(48.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .px_4()
                    .border_b_1()
                    .border_color(theme::border())
                    .text_color(rgb(theme::TEXT_STRONG))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(div().truncate().child(space.name.clone())),
            )
            .child(
                uniform_list(
                    "channels",
                    space.rows.len(),
                    cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                        range
                            .map(|ix| this.render_channel_row(ix, cx))
                            .collect::<Vec<_>>()
                    }),
                )
                .flex_1()
                .pt_2()
                .pb(px(PANEL_CLEARANCE)),
            )
    }

    fn render_channel_row(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let row = div().id(ix).h(px(34.)).mx_2().px_2().flex().items_center();
        match &self.spaces[self.selected_space].rows[ix] {
            Row::Friends => {
                let selected = self.friends_open;
                let pending = self.incoming_requests();
                row.gap_3()
                    .rounded_lg()
                    .cursor_pointer()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(if selected { theme::TEXT_STRONG } else { theme::CHANNELS_DEFAULT }))
                    .when(selected, |r| r.bg(theme::selected()))
                    .when(!selected, |r| {
                        r.hover(|s| s.bg(theme::hover()).text_color(rgb(theme::TEXT_STRONG)))
                    })
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
                .pb(px(6.))
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(theme::CHANNELS_DEFAULT))
                .child(div().truncate().child(title.clone()))
                .into_any_element(),
            Row::Channel { id, icon, name } => {
                let id = *id;
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
                    .child(
                        div()
                            .flex_none()
                            .w(px(20.))
                            .flex()
                            .justify_center()
                            .text_size(px(20.))
                            .font_weight(FontWeight::NORMAL)
                            .text_color(rgb(icon_color))
                            .child(*icon),
                    )
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

    fn render_message(&self, ix: usize) -> AnyElement {
        let Some(log) = self.selected_channel.and_then(|c| self.logs.get(&c)) else {
            return div().into_any_element();
        };
        let Some(msg) = log.messages.get(ix) else {
            return div().into_any_element();
        };
        let body = div()
            .text_color(theme::text())
            .child(msg.content.clone());
        let row = div()
            .id(("message", ix))
            .hover(|s| s.bg(theme::hover()));

        if self.settings.compact {
            // Compact mode: one line per message, no avatars or group gaps.
            return row
                .flex()
                .gap_2()
                .px_4()
                .py(px(1.))
                .child(
                    div()
                        .flex_none()
                        .text_xs()
                        .text_color(theme::muted())
                        .child(msg.time.clone()),
                )
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .min_w(px(0.))
                        .gap_1()
                        .child(
                            div()
                                .flex_none()
                                .text_color(rgb(theme::TEXT_STRONG))
                                .font_weight(FontWeight::MEDIUM)
                                .child(msg.author.clone()),
                        )
                        .child(body),
                )
                .into_any_element();
        }

        let grouped = ix > 0 && log.messages[ix - 1].author_id == msg.author_id;
        if grouped {
            return row
                .pl(px(72.))
                .pr_4()
                .py(px(2.))
                .child(body)
                .into_any_element();
        }

        // Group start: 17px gap above, outside the hover highlight.
        div()
            .pt(px(15.))
            .child(
                row.flex()
                    .gap_4()
                    .px_4()
                    .py(px(2.))
                    .child(
                        div()
                            .mt(px(2.))
                            .child(widgets::avatar(
                                &msg.author,
                                msg.author_id,
                                self.avatar_image(&msg.avatar_url),
                                40.,
                                None,
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(px(0.))
                            .child(
                                div()
                                    .flex()
                                    .gap(px(6.))
                                    .items_center()
                                    .child(
                                        div()
                                            .text_color(rgb(theme::TEXT_STRONG))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(msg.author.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme::muted())
                                            .child(msg.time.clone()),
                                    ),
                            )
                            .child(body),
                    ),
            )
            .into_any_element()
    }

    fn render_chat(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.friends_open && self.spaces[self.selected_space].id == DMS {
            return self.render_friends(cx).into_any_element();
        }
        let (icon, title) = self
            .selected_channel
            .and_then(|id| self.spaces[self.selected_space].channel(id))
            .unwrap_or(("", SharedString::default()));
        let loading = self
            .selected_channel
            .and_then(|id| self.logs.get(&id))
            .is_some_and(|log| log.loading);

        let body = if self.selected_channel.is_none() {
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme::muted())
                .child("Escolha um canal")
                .into_any_element()
        } else {
            list(
                self.messages.clone(),
                cx.processor(|this, ix, _, _| this.render_message(ix)),
            )
            .flex_1()
            .pb_4()
            // Discord's 1.375 line height, scaled with the chosen font size.
            .text_size(px(self.settings.font_size))
            .line_height(px((self.settings.font_size * 1.375).round()))
            .into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .bg(rgb(theme::BASE_LOWER))
            .border_l_1()
            .border_color(theme::border())
            .child(
                div()
                    .h(px(48.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_4()
                    .border_b_1()
                    .border_color(theme::border())
                    .child(
                        div()
                            .text_size(px(22.))
                            .text_color(rgb(theme::CHANNELS_DEFAULT))
                            .child(icon),
                    )
                    .child(
                        div()
                            .text_color(rgb(theme::TEXT_STRONG))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .when(loading, |d| {
                        d.child(
                            div()
                                .text_xs()
                                .text_color(theme::muted())
                                .child("carregando…"),
                        )
                    }),
            )
            .child(body)
            .child(div().flex_none().px_4().pb_6().child(self.input.clone()))
            .into_any_element()
    }
}

impl Focusable for Emerald {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Emerald {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Visual-refresh frame: server list on the app background, then one
        // bordered panel with a rounded top-left corner holding sidebar and
        // chat, and the user panel floating over the bottom-left corner.
        let overlay = match &self.overlay {
            Some(Overlay::AddServer(modal)) => Some(self.render_add_server(modal, cx).into_any_element()),
            Some(Overlay::Settings(view)) => Some(self.render_settings(view, cx).into_any_element()),
            None => None,
        };
        div()
            .track_focus(&self.focus_handle)
            .key_context("Emerald")
            .on_action(cx.listener(|this, _: &Dismiss, window, cx| this.close_overlay(window, cx)))
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
                    .child(self.render_chat(cx)),
            )
            .child(self.render_user_panel(cx))
            .children(overlay)
    }
}

fn dm_space(mut channels: Vec<Channel>, users: &HashMap<Snowflake, User>) -> Space {
    channels.retain(|c| matches!(c.kind, channel_kind::DM | channel_kind::GROUP_DM));
    for channel in &mut channels {
        channel.name = Some(dm_name(channel, users));
        channel.recipients = Vec::new();
        channel.recipient_ids = Vec::new();
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
