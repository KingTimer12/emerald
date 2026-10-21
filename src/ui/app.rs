//! The main window. `Emerald` composes the stores in [`state`], routes
//! network events to them ([`events`]) and draws them; each area of the
//! window lives in its own module.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

mod add_server;
mod attachments;
mod chat;
mod composer;
mod events;
mod friends;
mod images;
mod members;
mod navigation;
mod outbox;
mod profile;
mod settings;
mod sidebar;
mod state;
mod user_panel;
mod widgets;

use gpui::{
    App, Context, Entity, FocusHandle, Focusable, Image, KeyBinding, ListAlignment, ListState,
    SharedString, Subscription, UniformListScrollHandle, Window, actions, div, prelude::*, px, relative, rgb,
};

use super::text_input::{PastedImage, Submitted, TextInput};
use super::theme;
use crate::discord::model::{Profile, Snowflake};
use crate::discord::{Action, Command, Link};
use crate::settings::Settings;
use add_server::AddServer;
use composer::Composer;
use friends::Friends;
use images::Images;
pub use outbox::Outbox;
use settings::SettingsView;
use state::{History, MemberLists, MemberSearch, MessageView, People, ReadStates, Spaces, TtlCache, Typing};

actions!(emerald_ui, [Dismiss, EditLast]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", Dismiss, Some("Emerald")),
        KeyBinding::new("up", EditLast, Some("Emerald")),
    ]);
}

/// A cached profile is refetched when opened after this long.
const PROFILE_TTL: Duration = Duration::from_secs(10 * 60);
/// The profile panel only fits beside the chat in wide windows.
const PROFILE_MIN_WINDOW: f32 = 1100.;

/// Something drawn above the main layout; Escape dismisses it.
enum Overlay {
    AddServer(AddServer),
    Settings(SettingsView),
    /// "View Full Profile" for this user, as members of `guild` see them.
    Profile { user: Snowflake, guild: Option<Snowflake> },
    /// "Delete Message", asking before it goes.
    DeleteMessage { channel: Snowflake, message: Snowflake },
    /// "Modify Attachment" for the composer's file at `ix`; `spoiler` is the
    /// switch as set so far.
    EditAttachment { ix: usize, spoiler: bool },
    CreateThread(chat::NewThread),
    CreatePost(chat::NewPost),
}

/// Result line under a form, green on success and red on failure.
struct Feedback {
    ok: bool,
    text: SharedString,
}

impl Feedback {
    fn from_result(result: Result<(), String>, success: impl Into<SharedString>) -> Self {
        match result {
            Ok(()) => Self {
                ok: true,
                text: success.into(),
            },
            Err(error) => Self {
                ok: false,
                text: error.into(),
            },
        }
    }
}

/// The gateway connection as the user panel shows it.
struct Connection {
    /// The last state the gateway reported.
    link: Link,
    status: SharedString,
    /// `status` is an error or "logging out" rather than the link's state.
    custom: bool,
    /// Session is live; drives the user panel's status dot.
    connected: bool,
    fatal: Option<SharedString>,
}

impl Connection {
    fn set_link(&mut self, link: Link) {
        self.link = link;
        self.custom = false;
        self.connected = link == Link::Connected;
        self.status = match link {
            Link::Connecting => t!("connection.connecting").into(),
            Link::Resuming => t!("connection.resuming").into(),
            Link::Connected => t!("connection.connected").into(),
            Link::Reconnecting(secs) => t!("connection.reconnecting", secs = secs).into(),
        };
    }

    /// Shows a message other than the link's state.
    fn set_message(&mut self, message: impl Into<SharedString>) {
        self.status = message.into();
        self.custom = true;
    }

    /// Words the link's state again, in a new language.
    fn relabel(&mut self) {
        if !self.custom {
            self.set_link(self.link);
        }
    }
}

pub struct Emerald {
    net: Box<dyn Outbox>,
    settings: Settings,
    connection: Connection,
    people: People,
    images: Images,
    /// Animated stickers play on this clock, so copies of one stay in step.
    animation_clock: Instant,
    /// A row drawn this frame shows an animation, so another frame follows.
    animating: Cell<bool>,
    /// The next animation frame is already on its way.
    frame_pending: bool,
    /// Profiles, global or per guild, refetched after `PROFILE_TTL`.
    profiles: TtlCache<(Snowflake, Option<Snowflake>), Profile>,
    spaces: Spaces,
    history: History,
    reads: ReadStates,
    typing: Typing,
    /// The chat column shows the friends list instead of a channel.
    friends_open: bool,
    selected_channel: Option<Snowflake>,
    friends: Friends,
    composer: Composer,
    /// The own message being edited in place, and its edit box.
    edit: Option<chat::Editing>,
    edit_input: Entity<TextInput>,
    /// Alt text in "Modify Attachment".
    alt_input: Entity<TextInput>,
    message_menu: Option<chat::MessageMenu>,
    reaction_picker: Option<chat::ReactionPicker>,
    /// Forums opened this session, by channel.
    forums: HashMap<Snowflake, chat::Forum>,
    /// Spoiler images the user clicked open, by URL.
    revealed: HashSet<SharedString>,
    /// The reaction picker's search box, and its grid's scroll position.
    emoji_query: Entity<TextInput>,
    emoji_scroll: UniformListScrollHandle,
    /// Filters the DM list by name; enter opens the first match.
    dm_search: Entity<TextInput>,
    overlay: Option<Overlay>,
    /// REST actions in flight, one per kind.
    busy: HashSet<Action>,
    /// Select the next new guild, after a join or create succeeds.
    select_new_guild: bool,
    /// The open channel's messages, mirrored from `history`.
    messages: ListState,
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
        settings: Settings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let composer = Composer::new(cx);
        let friends = Friends::new(cx);
        let dm_search = cx.new(|cx| {
            TextInput::new(t!("dm.search_placeholder"), cx)
                .keep_on_submit()
                .compact()
        });
        let member_query = cx.new(|cx| TextInput::new(t!("members.search"), cx).keep_on_submit().compact());
        let edit_input = cx.new(|cx| TextInput::new("", cx));
        let alt_input = cx.new(|cx| TextInput::new(t!("upload.description_placeholder"), cx).keep_on_submit().submit_empty());
        let emoji_query = cx.new(|cx| TextInput::new(t!("reaction.search"), cx).keep_on_submit().compact());
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
            cx.subscribe(&friends.input, |this, _, event: &Submitted, cx| {
                this.send_friend_request(event.0.clone(), cx)
            }),
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
            net: Box::new(net),
            settings,
            connection: Connection {
                link: Link::Connecting,
                status: t!("connection.connecting").into(),
                custom: false,
                connected: false,
                fatal: None,
            },
            people: People::default(),
            images: Images::default(),
            animation_clock: Instant::now(),
            animating: Cell::new(false),
            frame_pending: false,
            profiles: TtlCache::new(PROFILE_TTL),
            spaces: Spaces::default(),
            history: History::default(),
            reads: ReadStates::default(),
            typing: Typing::default(),
            friends_open: true,
            selected_channel: None,
            friends,
            composer,
            edit: None,
            edit_input,
            alt_input,
            message_menu: None,
            reaction_picker: None,
            revealed: HashSet::new(),
            forums: HashMap::new(),
            emoji_query,
            emoji_scroll: UniformListScrollHandle::new(),
            dm_search,
            overlay: None,
            busy: HashSet::new(),
            select_new_guild: false,
            messages,
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

    /// Avatar of a known user, or the default one their id maps to.
    fn user_avatar(&self, id: Snowflake) -> Option<Arc<Image>> {
        self.avatar_image(&self.people.avatar_url(id))
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
            crate::sign_out();
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

impl Render for Emerald {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let wide = f32::from(window.viewport_size().width) >= PROFILE_MIN_WINDOW;
        // Visual-refresh frame: server list on the app background, then one
        // bordered panel with a rounded top-left corner holding sidebar and
        // chat, and the user panel floating over the bottom-left corner.
        let overlay = match &self.overlay {
            Some(Overlay::AddServer(modal)) => Some(self.render_add_server(modal, cx).into_any_element()),
            Some(Overlay::Settings(view)) => Some(self.render_settings(view, cx).into_any_element()),
            Some(Overlay::Profile { user, guild }) => {
                Some(self.render_profile_modal(*user, *guild, cx).into_any_element())
            }
            Some(Overlay::DeleteMessage { channel, message }) => {
                Some(self.render_delete_modal(*channel, *message, cx).into_any_element())
            }
            Some(Overlay::EditAttachment { ix, spoiler }) => self.render_attachment_modal(*ix, *spoiler, cx),
            Some(Overlay::CreateThread(modal)) => Some(self.render_thread_modal(modal, cx)),
            Some(Overlay::CreatePost(modal)) => Some(self.render_post_modal(modal, cx)),
            None => None,
        };
        let menu = self.render_message_menu(cx);
        let picker = self.render_reaction_picker(cx);
        div()
            .track_focus(&self.focus_handle)
            .key_context("Emerald")
            .on_action(cx.listener(|this, _: &Dismiss, window, cx| this.dismiss(window, cx)))
            .on_action(cx.listener(|this, _: &EditLast, window, cx| this.edit_last(window, cx)))
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
            .children(menu)
            .children(picker)
    }
}
