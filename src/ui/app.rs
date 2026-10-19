//! The main window. `Emerald` composes the stores in [`state`], routes
//! network events to them ([`events`]) and draws them; each area of the
//! window lives in its own module.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

mod add_server;
mod chat;
mod composer;
mod events;
mod friends;
mod images;
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
    SharedString, Subscription, Window, actions, div, prelude::*, px, relative, rgb,
};

use super::text_input::{Submitted, TextInput};
use super::theme;
use crate::discord::model::{Profile, Snowflake};
use crate::discord::{Action, Command};
use crate::settings::Settings;
use add_server::AddServer;
use composer::Composer;
use friends::Friends;
use images::Images;
pub use outbox::Outbox;
use settings::SettingsView;
use state::{History, MessageView, People, ReadStates, Spaces, TtlCache, Typing};

actions!(emerald_ui, [Dismiss]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", Dismiss, Some("Emerald"))]);
}

/// Status text the network layer reports once the session is live.
const CONNECTED: &str = "Conectado";
/// A cached profile is refetched when opened after this long.
const PROFILE_TTL: Duration = Duration::from_secs(10 * 60);
/// The profile panel only fits beside the chat in wide windows.
const PROFILE_MIN_WINDOW: f32 = 1100.;

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
    status: SharedString,
    /// Session is live; drives the user panel's status dot.
    connected: bool,
    fatal: Option<SharedString>,
}

impl Connection {
    fn set_status(&mut self, status: String) {
        self.connected = status == CONNECTED;
        self.status = status.into();
    }
}

pub struct Emerald {
    net: Box<dyn Outbox>,
    settings: Settings,
    connection: Connection,
    people: People,
    images: Images,
    /// DM profiles, refetched after `PROFILE_TTL`.
    profiles: TtlCache<Snowflake, Profile>,
    spaces: Spaces,
    history: History,
    reads: ReadStates,
    typing: Typing,
    /// The chat column shows the friends list instead of a channel.
    friends_open: bool,
    selected_channel: Option<Snowflake>,
    friends: Friends,
    composer: Composer,
    /// Filters the DM list by name; enter opens the first match.
    dm_search: Entity<TextInput>,
    overlay: Option<Overlay>,
    /// REST actions in flight, one per kind.
    busy: HashSet<Action>,
    /// Select the next new guild, after a join or create succeeds.
    select_new_guild: bool,
    /// The open channel's messages, mirrored from `history`.
    messages: ListState,
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
            TextInput::new("Encontre ou comece uma conversa", cx)
                .keep_on_submit()
                .compact()
        });
        let subscriptions = vec![
            cx.subscribe(&composer.input, |this, _, event: &Submitted, cx| {
                this.send(event.0.clone(), cx)
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
                status: "Conectando…".into(),
                connected: false,
                fatal: None,
            },
            people: People::default(),
            images: Images::default(),
            profiles: TtlCache::new(PROFILE_TTL),
            spaces: Spaces::default(),
            history: History::default(),
            reads: ReadStates::default(),
            typing: Typing::default(),
            friends_open: true,
            selected_channel: None,
            friends,
            composer,
            dm_search,
            overlay: None,
            busy: HashSet::new(),
            select_new_guild: false,
            messages,
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
        self.connection.status = "Saindo…".into();
        self.connection.connected = false;
        cx.notify();
        if !self.net.post(Command::LogOut) {
            crate::sign_out();
        }
    }

    fn save_settings(&mut self, cx: &mut Context<Self>) {
        if let Err(e) = self.settings.save() {
            log::error!("could not save settings: {e}");
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
