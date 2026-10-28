//! The suggestion list over the message box: members, roles, channels and
//! emoji for the word being typed, picked with the arrows and Tab or Enter,
//! or the mouse. [`state::autocomplete`] decides what is being completed;
//! this builds the entries and draws them.

mod build;
mod render;

use std::time::{Duration, Instant};

use gpui::{AnyElement, Context, FontWeight, SharedString, div, prelude::*, relative, rgb};

use super::super::text_input::MenuKey;
use super::super::{emoji, icons, theme};
use super::state::autocomplete::{self, Completion, Query, Trigger};
use super::{Emerald, widgets};
use emerald_discord::model::{AppCommand, ChannelKind, Emoji, Snowflake};
use emerald_discord::{Command, permissions};

/// The web client waits this long after a keystroke before asking the
/// gateway for members.
const QUERY_DELAY: Duration = Duration::from_millis(200);
/// And asks for the same name in the same guild at most once a minute.
const QUERY_TTL: Duration = Duration::from_secs(60);
const ROW: f32 = 40.;
/// Roles and `@everyone` come after up to ten members, a few at most.
const ROLES: usize = 5;

/// One entry of the list.
pub(super) enum Suggestion {
    Member {
        id: Snowflake,
        name: SharedString,
        username: SharedString,
        color: Option<u32>,
    },
    Role {
        id: Snowflake,
        name: SharedString,
        color: Option<u32>,
    },
    /// `@everyone` or `@here`.
    Everyone(&'static str),
    Channel {
        id: Snowflake,
        name: SharedString,
        icon: &'static str,
    },
    Emoji {
        emoji: Emoji,
        name: SharedString,
    },
    /// A slash command, and the app it belongs to.
    Command {
        command: Box<AppCommand>,
        app: SharedString,
    },
}

impl Suggestion {
    /// What a pick puts in the box, and what is sent in its place.
    fn completion(&self) -> Completion {
        let (shown, sent) = match self {
            Self::Member { id, name, .. } => (format!("@{name}"), Some(format!("<@{id}>"))),
            Self::Role { id, name, .. } => (format!("@{name}"), Some(format!("<@&{id}>"))),
            Self::Everyone(word) => (format!("@{word}"), None),
            Self::Channel { id, name, .. } => (format!("#{name}"), Some(format!("<#{id}>"))),
            Self::Emoji { emoji, name } => match emoji.id {
                Some(id) => {
                    let prefix = if emoji.animated { "a" } else { "" };
                    (format!(":{name}:"), Some(format!("<{prefix}:{name}:{id}>")))
                }
                // Unicode emoji go in as themselves.
                None => (emoji.name.clone().unwrap_or_default(), None),
            },
            Self::Command { command, .. } => (format!("/{}", command.name), None),
        };
        Completion { shown, sent }
    }

    /// The heading of the group the entry belongs to.
    fn section(&self) -> &'static str {
        match self {
            Self::Member { .. } => t!("autocomplete.members"),
            Self::Role { .. } | Self::Everyone(_) => t!("autocomplete.roles"),
            Self::Channel { .. } => t!("autocomplete.channels"),
            Self::Emoji { .. } => t!("autocomplete.emoji"),
            Self::Command { .. } => t!("autocomplete.commands"),
        }
    }
}

impl Emerald {
    /// Follows the word at the cursor; rebuilds the list when it changed,
    /// or always when `force` (new members arrived).
    pub(super) fn refresh_suggestions(&mut self, force: bool, cx: &mut Context<Self>) {
        let input = self.composer.input.read(cx);
        let (text, cursor, composing) = (input.text().to_owned(), input.cursor(), input.is_composing());
        let changed = if composing {
            false
        } else {
            self.autocomplete.update(&text, cursor)
        };
        if !changed && !force {
            return;
        }
        let query = self.autocomplete.query().cloned();
        self.suggestions = query.as_ref().map(|q| self.build_suggestions(q)).unwrap_or_default();
        self.autocomplete.set_len(self.suggestions.len());
        match query.filter(|_| changed).map(|q| (q.trigger, q.text)) {
            Some((Trigger::Mention, text)) => self.query_members(text, cx),
            Some((Trigger::Command, _)) => self.request_commands(),
            _ => {}
        }
        self.sync_menu(cx);
    }

    /// Tells the input whether its arrows and Enter belong to the list.
    fn sync_menu(&mut self, cx: &mut Context<Self>) {
        let open = self.autocomplete.is_open();
        self.composer.input.update(cx, |input, _| input.set_menu_open(open));
        cx.notify();
    }

    /// Asks the gateway for members named like `query`, as the web client
    /// does: after a pause in typing, and not again for a while.
    fn query_members(&mut self, query: String, cx: &mut Context<Self>) {
        let query = query.trim().to_lowercase();
        let Some(guild) = self.spaces.current().info().map(|_| self.spaces.current().id) else { return };
        if query.is_empty() {
            return;
        }
        let now = Instant::now();
        self.member_queries.retain(|(_, _, at)| now.duration_since(*at) < QUERY_TTL);
        if self.member_queries.iter().any(|(g, q, _)| *g == guild && *q == query) {
            return;
        }
        self.member_queries.push((guild, query.clone(), now));
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(QUERY_DELAY).await;
            this.update(cx, |this, _| {
                // Typing went on; the newer word asks for itself.
                let current = this.autocomplete.query().map(|q| q.text.trim().to_lowercase());
                if current.as_deref() == Some(query.as_str()) {
                    this.net.post(Command::QueryMembers { guild, query });
                } else {
                    this.member_queries.retain(|(g, q, _)| !(*g == guild && *q == query));
                }
            })
            .ok();
        })
        .detach();
    }

    /// A key the input handed over while the list is open.
    pub(super) fn on_menu_key(&mut self, key: MenuKey, cx: &mut Context<Self>) {
        match key {
            MenuKey::Up => self.autocomplete.step(-1),
            MenuKey::Down => self.autocomplete.step(1),
            MenuKey::Accept => return self.accept_suggestion(self.autocomplete.selected(), cx),
        }
        cx.notify();
    }

    /// Escape closes the list before anything else.
    pub(super) fn dismiss_suggestions(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.autocomplete.dismiss() {
            return false;
        }
        self.sync_menu(cx);
        true
    }

    /// Puts entry `ix` in place of the word being typed.
    fn accept_suggestion(&mut self, ix: usize, cx: &mut Context<Self>) {
        let (Some(suggestion), Some(query)) = (self.suggestions.get(ix), self.autocomplete.query().cloned()) else {
            return;
        };
        let completion = suggestion.completion();
        if let Suggestion::Command { command, .. } = suggestion {
            self.composer.command = Some((**command).clone());
        }
        self.insert_completion(query.range, completion, cx);
    }

    /// An emoji from the picker beside the message box, at the cursor.
    pub(super) fn insert_emoji(&mut self, emoji: Emoji, cx: &mut Context<Self>) {
        let name = emoji.name.clone().unwrap_or_default().into();
        let completion = Suggestion::Emoji { emoji, name }.completion();
        let cursor = self.composer.input.read(cx).cursor();
        self.insert_completion(cursor..cursor, completion, cx);
    }

    /// Writes `completion` over `range` of the box, then a space, and keeps
    /// it to turn into its mention on send.
    fn insert_completion(&mut self, range: std::ops::Range<usize>, mut completion: Completion, cx: &mut Context<Self>) {
        // Two picks that look the same but mean different things: this one
        // goes in as its mention, so neither is mistaken for the other.
        let clash = self
            .composer
            .completions
            .iter()
            .any(|c| c.shown == completion.shown && c.sent != completion.sent);
        if clash && let Some(sent) = completion.sent.take() {
            completion.shown = sent;
        }
        let inserted = format!("{} ", completion.shown);
        if completion.sent.is_some() && !self.composer.completions.contains(&completion) {
            self.composer.completions.push(completion);
        }
        self.autocomplete.close();
        self.suggestions.clear();
        self.composer.input.update(cx, |input, cx| {
            input.set_menu_open(false);
            input.replace(range, &inserted, cx);
        });
        cx.notify();
    }
}
