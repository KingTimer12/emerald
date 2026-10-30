//! The main window. `Emerald` composes the stores in [`state`], routes
//! network events to them ([`events`]) and draws them; each area of the
//! window lives in its own module.

use std::cell::Cell;
use std::rc::Rc;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};
use crate::zoom::zoomed;

mod account;
mod activity;
mod add_server;
mod alerts;
mod back_forward;
mod attachments;
mod chat;
mod composer;
mod connection;
mod deep_links;
mod desktop;
mod dialogs;
mod events;
mod friends;
mod group_members;
mod group_picker;
mod images;
mod invites;
mod members;
mod channel_settings;
mod moderation;
mod navigation;
mod outbox;
mod place_actions;
mod place_menu;
mod panels;
mod profile;
mod render;
mod requests;
mod settings;
mod sidebar;
mod slash;
mod state;
mod suggestions;
mod switcher;
mod title_bar;
mod user_menu;
mod user_panel;
mod voice;
mod widgets;
mod window;
pub use window::initial_bounds;

use connection::*;

use gpui::{
    App, Context, Entity, FocusHandle, Focusable, Image, ImageSource, KeyBinding, KeyDownEvent, ListAlignment, ListState,
    SharedString, Subscription, UniformListScrollHandle, Window, actions, div, prelude::*, relative, rgb,
};

use crate::shortcuts::{EditLast, GoBack, GoForward, MarkRead, NextChannel, NextUnread, PreviousChannel, QuickSwitch};

use super::text_input::{MenuKey, PastedImage, Submitted, TextInput};
use super::{theme, time};
use emerald_discord::model::{AvatarDecoration, Profile, ProfileEffect, Snowflake};
use emerald_discord::{Action, Command, Link};
use rustc_hash::{FxHashMap, FxHashSet};
use crate::settings::Settings;
use add_server::AddServer;
use composer::Composer;
use friends::Friends;
use images::{Images, Still};
pub use alerts::Alerts;
pub use desktop::{Desktop, Update};
pub use outbox::Outbox;
use settings::SettingsView;
use state::{
    Autocomplete, History, MemberLists, MemberSearch, MessageView, NotifySettings, People, ReadStates, ServerList, Spaces,
    TtlCache, Typing,
};

actions!(emerald_ui, [Dismiss, ZoomIn, ZoomOut, ZoomReset]);

/// Escape, Copy and zoom; the other keys are in `shortcuts`. Zoom keys are
/// fixed, as in other apps, so they work whatever the shortcuts say.
pub(crate) fn bind_fixed_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", Dismiss, Some("Emerald")),
        // Selected message text; inputs handle it first when they have a selection.
        KeyBinding::new("secondary-c", crate::text_input::Copy, Some("Emerald")),
        KeyBinding::new("secondary-=", ZoomIn, Some("Emerald")),
        KeyBinding::new("secondary--", ZoomOut, Some("Emerald")),
        KeyBinding::new("secondary-0", ZoomReset, Some("Emerald")),
    ]);
}

/// A cached profile is refetched when opened after this long.
const PROFILE_TTL: Duration = Duration::from_secs(10 * 60);
/// Profile effects remembered; past this the map starts over.
const MAX_PROFILE_EFFECTS: usize = 256;
/// The profile panel only fits beside the chat in wide windows.
const PROFILE_MIN_WINDOW: f32 = 1100.;
/// The member list is narrower: server list, sidebar and the list (72 +
/// 240 + 240) still leave the chat about 400px.
const MEMBERS_MIN_WINDOW: f32 = 960.;
/// Avatars are fetched this many pixels square (`?size=128`).
const AVATAR_PIXELS: u32 = 128;

/// Something drawn above the main layout; Escape dismisses it.
enum Overlay {
    AddServer(AddServer),
    /// Boxed: the settings hold forms far larger than any other overlay.
    Settings(Box<SettingsView>),
    /// "View Full Profile" for this user, as members of `guild` see them.
    Profile { user: Snowflake, guild: Option<Snowflake> },
    /// "Delete Message", asking before it goes.
    DeleteMessage { channel: Snowflake, message: Snowflake },
    /// "Modify Attachment" for the composer's file at `ix`; `spoiler` is the
    /// switch as set so far.
    EditAttachment { ix: usize, spoiler: bool },
    CreateThread(chat::NewThread),
    CreatePost(chat::NewPost),
    CreatePoll(chat::NewPoll),
    ChannelSettings(Box<channel_settings::ChannelSettingsModal>),
    /// The quick switcher (Ctrl/Cmd+K).
    Switcher(switcher::Switcher),
    /// A question before leaving, blocking or removing someone.
    Confirm(dialogs::Ask),
    /// A name to type: a group's, a friend's nickname.
    Prompt(dialogs::Prompt),
    /// Friends to start a group with, or to add to a DM.
    GroupPicker(group_picker::GroupPicker),
    /// "Invite People".
    Invite(invites::InviteModal),
}

pub struct Emerald {
    net: Box<dyn Outbox>,
    /// Desktop notifications and the count on the app icon.
    alerts: Box<dyn Alerts>,
    /// The count last put on the app icon.
    badge_shown: Option<u32>,
    /// Opening on login and updates.
    desktop: Box<dyn Desktop>,
    update: Update,
    /// Read when the Desktop App settings open.
    opens_on_login: Option<bool>,
    /// A link the app was opened with before the first READY.
    pending_link: Option<deep_links::DeepLink>,
    /// Mutes, notification levels and folded categories, synced with Discord.
    notify: NotifySettings,
    /// The server list's folders and order.
    server_list: ServerList,
    settings: Settings,
    connection: Connection,
    people: People,
    /// The account's synced settings, presence and devices.
    account: state::Account,
    images: Images,
    /// Animated stickers play on this clock, so copies of one stay in step.
    animation_clock: Instant,
    /// A row drawn this frame shows an animation, so another frame follows.
    animating: Cell<bool>,
    /// The next animation frame is already on its way.
    frame_pending: bool,
    /// Profiles, global or per guild, refetched after `PROFILE_TTL`.
    profiles: TtlCache<(Snowflake, Option<Snowflake>), Profile>,
    /// Profile effect stills by SKU; `None` while asked for or when there
    /// is none. Cleared past [`MAX_PROFILE_EFFECTS`].
    profile_effects: FxHashMap<String, Option<Rc<ProfileEffect>>>,
    /// When the profile last opened: its effect plays from there.
    profile_opened: Instant,
    /// The member list row under the pointer, whose cosmetics play.
    hovered_member: Option<Snowflake>,
    spaces: Spaces,
    history: History,
    reads: ReadStates,
    typing: Typing,
    /// Channels opened lately, offered first by the quick switcher.
    recent: state::switcher::Recent,
    /// Places opened, for the title bar's back and forward arrows.
    places: state::NavHistory,
    /// Set while back or forward opens a place, so it is not visited again.
    walking_history: bool,
    /// The chat column shows the friends list instead of a channel.
    friends_open: bool,
    /// The chat column shows the message requests, or the spam inbox.
    requests_open: bool,
    requests_spam: bool,
    selected_channel: Option<Snowflake>,
    friends: Friends,
    composer: Composer,
    /// The suggestions over the message box, and their entries.
    autocomplete: Autocomplete,
    suggestions: Vec<suggestions::Suggestion>,
    /// `@` names asked of the gateway lately, per guild, to ask once a minute.
    member_queries: Vec<(Snowflake, String, Instant)>,
    /// Discord's standard sticker packs; `Some` once asked for, and filled
    /// when they arrive.
    sticker_packs: Option<Vec<emerald_discord::model::StickerPack>>,
    /// Slash commands per guild or DM, and the user's own apps'.
    command_sets: FxHashMap<emerald_discord::IndexTarget, slash::CommandSet>,
    /// The own message being edited in place, and its edit box.
    edit: Option<chat::Editing>,
    edit_input: Entity<TextInput>,
    /// Alt text in "Modify Attachment".
    alt_input: Entity<TextInput>,
    message_menu: Option<chat::MessageMenu>,
    reaction_picker: Option<chat::ReactionPicker>,
    /// Pins, search results, the inbox or threads, beside the chat.
    panel: Option<panels::Panel>,
    /// The search box in the channel header.
    search_input: Entity<TextInput>,
    /// The search box of the threads panel.
    thread_query: Entity<TextInput>,
    /// The search box of an open forum.
    forum_query: Entity<TextInput>,
    /// The right-click menu of a server, category or channel.
    place_menu: Option<place_menu::PlaceMenu>,
    /// The right-click menu of a person.
    user_menu: Option<user_menu::UserMenu>,
    /// The text field of a name dialog.
    prompt_input: Entity<TextInput>,
    /// The search box of the friend picker.
    picker_query: Entity<TextInput>,
    /// The note on the open profile.
    note_input: Entity<TextInput>,
    /// Whose note `note_input` holds, to save it on Enter.
    note_user: Option<Snowflake>,
    /// The tab of the full profile.
    profile_tab: profile::ProfileTab,
    /// Friends in common, per user, once asked for.
    mutual_friends: FxHashMap<Snowflake, Vec<Snowflake>>,
    /// Invites linked in messages, resolved for their cards.
    invite_cards: invites::InviteCards,
    /// Runs of blocked or ignored messages opened, by their first message.
    revealed_hidden: FxHashSet<Snowflake>,
    /// Forums opened this session, by channel.
    forums: FxHashMap<Snowflake, chat::Forum>,
    /// Spoiler images the user clicked open, by URL.
    revealed: HashSet<SharedString>,
    /// The menu of the + beside the message box is open.
    plus_menu: bool,
    /// Answers picked on a poll before voting: (message, answers).
    poll_choice: Option<(Snowflake, Vec<u32>)>,
    /// Polls whose results the user asked to see before voting.
    poll_results: FxHashSet<Snowflake>,
    /// Text spoilers the user clicked open.
    revealed_spoilers: FxHashSet<chat::SpoilerKey>,
    /// The window has focus; animated emoji and avatars only play then,
    /// as in the web client, so the app idles when in the background.
    window_active: bool,
    /// The window place written to the settings after it settles.
    window_save: Option<gpui::Task<()>>,
    /// Messages picked for a bulk delete, see `chat::actions::bulk`.
    bulk: Option<chat::Selection>,
    /// The voice call we are in or joining.
    call: Option<state::Call>,
    /// Why the last call failed, shown where the call panel was.
    call_notice: Option<SharedString>,
    /// Calls going on in DMs, ours or not.
    dm_calls: state::DmCalls,
    /// This view, for handlers drawn deep in the text to call back.
    weak: gpui::WeakEntity<Emerald>,
    /// The reaction picker's search box, and its grid's scroll position.
    emoji_query: Entity<TextInput>,
    emoji_scroll: UniformListScrollHandle,
    /// Filters the DM list by name; enter opens the first match.
    dm_search: Entity<TextInput>,
    overlay: Option<Overlay>,
    /// REST actions in flight, one per kind.
    busy: FxHashSet<Action>,
    /// Select the next new guild, after a join or create succeeds.
    select_new_guild: bool,
    /// The open channel's messages, mirrored from `history`.
    messages: ListState,
    /// A message being loaded to jump to.
    jump: Option<chat::Jump>,
    /// The open channel had unread messages when opened: (channel, last
    /// read message), for the "new messages" divider and bar.
    unread_marker: Option<(Snowflake, Snowflake)>,
    /// The "new messages" divider was drawn, so its bar is not needed.
    divider_seen: bool,
    /// The message just jumped to, highlighted for a moment.
    flash: Option<Snowflake>,
    /// Text selected by dragging over a message.
    text_selection: Option<chat::TextSelection>,
    /// Member list of the open guild channel.
    members: MemberLists,
    /// A member list subscription waits out a scroll burst.
    members_resubscribe: bool,
    members_scroll: UniformListScrollHandle,
    /// Search box above the member list, and what it found.
    member_query: Entity<TextInput>,
    member_search: MemberSearch,
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl Emerald {
    pub fn new(
        net: impl Outbox + 'static,
        alerts: impl Alerts + 'static,
        desktop: impl Desktop + 'static,
        settings: Settings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // The network thread keeps them for the first call.
        net.post(Command::VoiceControls(settings.voice.controls()));
        let composer = Composer::new(cx);
        let friends = Friends::new(cx);
        let dm_search = cx.new(|cx| {
            TextInput::new(t!("dm.search_placeholder"), cx)
                .keep_on_submit()
                .compact()
        });
        let member_query = cx.new(|cx| TextInput::new(t!("members.search"), cx).keep_on_submit().compact());
        let edit_input = cx.new(|cx| TextInput::new("", cx).multiline(composer::MAX_ROWS));
        let alt_input = cx.new(|cx| TextInput::new(t!("upload.description_placeholder"), cx).keep_on_submit().submit_empty());
        let emoji_query = cx.new(|cx| TextInput::new(t!("reaction.search"), cx).keep_on_submit().compact());
        let search_input = cx.new(|cx| TextInput::new(t!("search.placeholder"), cx).keep_on_submit().compact());
        let thread_query = cx.new(|cx| TextInput::new(t!("threads.search"), cx).keep_on_submit().submit_empty().compact());
        let forum_query = cx.new(|cx| TextInput::new(t!("forum.search"), cx).keep_on_submit().submit_empty().compact());
        let prompt_input = cx.new(|cx| TextInput::new("", cx).keep_on_submit().submit_empty());
        let picker_query = cx.new(|cx| TextInput::new(t!("group.search"), cx).keep_on_submit().compact());
        let note_input = cx.new(|cx| TextInput::new(t!("note.placeholder"), cx).keep_on_submit().submit_empty().compact());
        let subscriptions = vec![
            cx.subscribe_in(&alt_input, window, |this, _, _: &Submitted, window, cx| {
                this.save_attachment(window, cx)
            }),
            cx.subscribe_in(&edit_input, window, |this, _, event: &Submitted, window, cx| {
                this.save_edit(event.0.clone(), window, cx)
            }),
            cx.observe(&member_query, |this, _, cx| this.on_member_query(cx)),
            cx.observe(&emoji_query, |this, _, cx| {
                this.emoji_scroll = UniformListScrollHandle::new();
                this.on_picker_query(cx);
                cx.notify();
            }),
            cx.subscribe_in(&emoji_query, window, |this, _, _: &Submitted, window, cx| {
                this.pick_first_emoji(window, cx)
            }),
            cx.subscribe(&composer.input, |this, _, event: &Submitted, cx| {
                this.send(event.0.clone(), cx)
            }),
            cx.subscribe(&composer.input, |this, _, event: &PastedImage, cx| {
                this.attach_pasted(&event.0, cx)
            }),
            cx.observe(&composer.input, |this, _, cx| this.on_draft_changed(cx)),
            cx.subscribe(&composer.input, |this, _, key: &MenuKey, cx| this.on_menu_key(*key, cx)),
            cx.subscribe(&search_input, |this, _, event: &Submitted, cx| this.submit_search(event.0.clone(), cx)),
            cx.subscribe(&thread_query, |this, _, event: &Submitted, cx| this.search_threads(event.0.clone(), cx)),
            cx.subscribe(&forum_query, |this, _, event: &Submitted, cx| this.search_forum(event.0.clone(), cx)),
            cx.subscribe_in(&prompt_input, window, |this, _, _: &Submitted, window, cx| this.submit_prompt(window, cx)),
            cx.observe(&picker_query, |_, _, cx| cx.notify()),
            cx.subscribe(&note_input, |this, _, event: &Submitted, cx| this.save_note(event.0.clone(), cx)),
            cx.subscribe(&friends.input, |this, _, event: &Submitted, cx| {
                this.send_friend_request(event.0.clone(), cx)
            }),
            cx.observe(&dm_search, |_, _, cx| cx.notify()),
            cx.observe_window_activation(window, |this, window, cx| {
                this.window_active = window.is_window_active();
                cx.notify();
            }),
            cx.observe_window_bounds(window, |this, window, cx| this.remember_window(window, cx)),
            cx.observe_window_appearance(window, |_, _, cx| cx.notify()),
            cx.subscribe_in(&dm_search, window, |this, _, _: &Submitted, window, cx| {
                this.open_first_dm(window, cx)
            }),
        ];

        let messages = ListState::new(0, ListAlignment::Bottom, zoomed(400.));
        let weak = cx.entity().downgrade();
        messages.set_scroll_handler(move |event, _, cx| {
            let (range, count) = (event.visible_range.clone(), event.count);
            let weak = weak.clone();
            // The list state is borrowed while this handler runs.
            cx.defer(move |cx| {
                weak.update(cx, |this, cx| {
                    if range.start == 0 && count > 0 {
                        this.load_older(cx);
                    }
                    this.on_list_scrolled(range.end, count);
                })
                .ok();
            });
        });

        Self {
            net: Box::new(net),
            alerts: Box::new(alerts),
            badge_shown: None,
            desktop: Box::new(desktop),
            update: Update::Idle,
            opens_on_login: None,
            pending_link: None,
            notify: NotifySettings::default(),
            server_list: ServerList::default(),
            settings,
            connection: Connection {
                link: Link::Connecting,
                status: t!("connection.connecting").into(),
                custom: false,
                connected: false,
                fatal: None,
            },
            people: People::default(),
            account: state::Account::default(),
            images: Images::default(),
            animation_clock: Instant::now(),
            animating: Cell::new(false),
            frame_pending: false,
            profiles: TtlCache::new(PROFILE_TTL),
            profile_effects: FxHashMap::default(),
            profile_opened: Instant::now(),
            hovered_member: None,
            spaces: Spaces::default(),
            history: History::default(),
            reads: ReadStates::default(),
            typing: Typing::default(),
            recent: Default::default(),
            // Home's friends list shows first, until a channel is restored.
            places: state::NavHistory::starting_at(state::VisitedPlace::Friends),
            walking_history: false,
            friends_open: true,
            requests_open: false,
            requests_spam: false,
            selected_channel: None,
            friends,
            composer,
            autocomplete: Autocomplete::default(),
            suggestions: Vec::new(),
            member_queries: Vec::new(),
            sticker_packs: None,
            command_sets: FxHashMap::default(),
            edit: None,
            edit_input,
            alt_input,
            message_menu: None,
            reaction_picker: None,
            revealed: HashSet::new(),
            revealed_spoilers: FxHashSet::default(),
            plus_menu: false,
            poll_choice: None,
            poll_results: FxHashSet::default(),
            window_active: window.is_window_active(),
            window_save: None,
            bulk: None,
            call: None,
            call_notice: None,
            dm_calls: Default::default(),
            weak: cx.entity().downgrade(),
            forums: FxHashMap::default(),
            panel: None,
            search_input,
            thread_query,
            forum_query,
            place_menu: None,
            user_menu: None,
            prompt_input,
            picker_query,
            note_input,
            note_user: None,
            profile_tab: profile::ProfileTab::About,
            mutual_friends: FxHashMap::default(),
            invite_cards: Default::default(),
            revealed_hidden: FxHashSet::default(),
            emoji_query,
            emoji_scroll: UniformListScrollHandle::new(),
            dm_search,
            overlay: None,
            busy: FxHashSet::default(),
            select_new_guild: false,
            messages,
            jump: None,
            unread_marker: None,
            divider_seen: false,
            flash: None,
            text_selection: None,
            members: MemberLists::default(),
            members_resubscribe: false,
            members_scroll: UniformListScrollHandle::new(),
            member_query,
            member_search: MemberSearch::default(),
            focus_handle: cx.focus_handle(),
            _subscriptions: subscriptions,
        }
    }

    pub fn input_focus(&self, cx: &App) -> FocusHandle {
        self.composer.input.focus_handle(cx)
    }

    /// The image at `url` if downloaded; otherwise starts the download (once).
    fn avatar_image(&self, url: &str) -> Option<Arc<Image>> {
        self.images.get(url, &*self.net)
    }

    /// A Nitro avatar decoration for an avatar `size` pixels square: moving
    /// when `moving` and the window animates, else still; `None` until
    /// downloaded.
    fn decoration_image(&self, decoration: &AvatarDecoration, size: f32, moving: bool) -> Option<ImageSource> {
        let now = time::now_secs();
        // 1.2 times the avatar, twice that for Retina.
        let pixels = (size * 2.4) as u32;
        let frames = (moving && self.animates())
            .then(|| decoration.url(now, true))
            .flatten()
            .and_then(|url| self.images.animated(&url, pixels, pixels, &*self.net));
        match frames {
            Some(frames) => Some(ImageSource::Render(frames)),
            None => self.avatar_image(&decoration.url(now, false)?).map(ImageSource::Image),
        }
    }

    /// The decoration around `user`'s avatar.
    fn decoration(&self, user: &emerald_discord::model::User, size: f32, moving: bool) -> Option<ImageSource> {
        self.decoration_image(user.avatar_decoration_data.as_ref()?, size, moving)
    }

    /// The same for a user known by id.
    fn decoration_of(&self, id: Snowflake, size: f32, moving: bool) -> Option<ImageSource> {
        self.people.user(id).and_then(|user| self.decoration(user, size, moving))
    }

    /// Avatar of a known user, or the default one their id maps to.
    fn user_avatar(&self, id: Snowflake) -> Option<Arc<Image>> {
        self.avatar_image(&self.people.avatar_url(id))
    }

    /// A user's avatar on their profile: moving when it is animated and
    /// the window has focus, as the web client plays them, else still.
    /// gpui draws only a WebP's first frame, so the moving one is decoded
    /// apart, at the size it is fetched.
    fn profile_avatar(&self, id: Snowflake) -> Option<ImageSource> {
        let animated = self
            .people
            .user(id)
            .filter(|_| self.animates())
            .and_then(|user| user.animated_avatar_url());
        animated
            .and_then(|url| self.images.animated(&url, AVATAR_PIXELS, AVATAR_PIXELS, &*self.net))
            .map(ImageSource::Render)
            .or_else(|| self.user_avatar(id).map(ImageSource::Image))
    }

    /// Sends a REST action unless the same kind is already in flight.
    fn run(&mut self, action: Action, command: Command, cx: &mut Context<Self>) -> bool {
        if !self.busy.insert(action) {
            return false;
        }
        self.net.post(command);
        cx.notify();
        true
    }

    fn close_overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.overlay.take().is_some() {
            window.focus(&self.composer.input.focus_handle(cx));
            cx.notify();
        }
    }

    /// Asks the network thread to revoke the token; `main` signs out locally
    /// when the answer arrives. Without a network thread, signs out at once.
    fn log_out(&mut self, cx: &mut Context<Self>) {
        self.connection.set_message(t!("connection.logging_out"));
        self.connection.connected = false;
        cx.notify();
        if !self.net.post(Command::LogOut) {
            emerald_discord::sign_out();
        }
    }

    fn save_settings(&mut self, cx: &mut Context<Self>) {
        if let Err(e) = self.settings.save() {
            tracing::error!("could not save settings: {e}");
        }
        cx.notify();
    }
}

/// The chat list when `channel` is the one on screen, for stores to keep in
/// step. A free function so it borrows only those two fields.
fn shown(messages: &ListState, selected: Option<Snowflake>, channel: Snowflake) -> Option<&dyn MessageView> {
    (selected == Some(channel)).then_some(messages as &dyn MessageView)
}

impl Focusable for Emerald {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
