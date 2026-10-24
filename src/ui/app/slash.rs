//! Slash commands from the message box: the command index is fetched the
//! first time `/` opens a message in a place, commands are offered like
//! other suggestions, and a picked command runs on Enter as an interaction.
//! Interactions are bot traffic, so nothing here runs on its own: one
//! Enter, one interaction.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use gpui::{AnyElement, Context, FontWeight, SharedString, div, prelude::*, px, rgb};

use super::{Emerald, widgets};
use super::state::autocomplete;
use super::state::commands::{self, CommandError};
use super::super::theme;
use crate::discord::model::{AppCommand, CHAT_INPUT, CommandIndex, Snowflake};
use crate::discord::{Command, IndexTarget, permissions, rest};

/// An index is fetched again when `/` is typed after this long.
const INDEX_TTL: Duration = Duration::from_secs(10 * 60);
/// Indexes kept, one per guild or DM where `/` was typed.
const MAX_INDEXES: usize = 16;

/// The commands of one guild, DM or the user's own apps.
pub(super) struct CommandSet {
    commands: Vec<AppCommand>,
    /// App names, by id, shown beside their commands.
    apps: HashMap<Snowflake, String>,
    /// When it was asked for; it may still be on its way.
    asked: Instant,
}

impl Emerald {
    /// Where the open channel's commands come from: its guild or the DM,
    /// and the user's own apps.
    fn command_targets(&self) -> Vec<IndexTarget> {
        let Some(channel) = self.selected_channel else { return Vec::new() };
        let space = self.spaces.current();
        let place = if space.is_dms() { IndexTarget::Channel(channel) } else { IndexTarget::Guild(space.id) };
        vec![place, IndexTarget::User]
    }

    /// Asks for the indexes not known or gone stale, once each.
    pub(super) fn request_commands(&mut self) {
        let now = Instant::now();
        for target in self.command_targets() {
            if self.command_sets.get(&target).is_some_and(|set| now.duration_since(set.asked) < INDEX_TTL) {
                continue;
            }
            if self.command_sets.len() >= MAX_INDEXES
                && let Some(oldest) = self.command_sets.iter().min_by_key(|(_, set)| set.asked).map(|(t, _)| *t)
            {
                self.command_sets.remove(&oldest);
            }
            let commands = self.command_sets.remove(&target).map(|set| set.commands).unwrap_or_default();
            self.command_sets.insert(target, CommandSet { commands, apps: HashMap::new(), asked: now });
            self.net.post(Command::ListCommands(target));
        }
    }

    pub(super) fn on_command_index(&mut self, target: IndexTarget, index: Option<CommandIndex>, cx: &mut Context<Self>) {
        let Some(set) = self.command_sets.get_mut(&target) else { return };
        match index {
            Some(index) => {
                set.apps = index.applications.into_iter().map(|app| (app.id, app.name)).collect();
                set.commands = index
                    .application_commands
                    .into_iter()
                    .filter_map(AppCommand::parse)
                    .filter(|c| c.kind == CHAT_INPUT)
                    .collect();
            }
            // Asked again on the next `/`.
            None => {
                set.asked = Instant::now() - INDEX_TTL;
            }
        }
        if self.autocomplete.query().is_some_and(|q| q.trigger == autocomplete::Trigger::Command) {
            self.refresh_suggestions(true, cx);
        }
    }

    /// Commands named like `text` that may run here, with their app's name.
    pub(super) fn command_suggestions(&self, text: &str) -> Vec<(AppCommand, SharedString)> {
        let channel_permissions = self.selected_channel.map_or(0, |c| self.spaces.current().permissions(c));
        let admin = channel_permissions & permissions::ADMINISTRATOR != 0;
        let mut found: Vec<(AppCommand, SharedString)> = Vec::new();
        for target in self.command_targets() {
            let Some(set) = self.command_sets.get(&target) else { continue };
            for command in &set.commands {
                // Commands meant for members with more rights stay hidden, as
                // in the web client. Admins of the server can still change
                // that per command; this does not know about such changes.
                let allowed = command
                    .default_member_permissions
                    .is_none_or(|needed| admin || channel_permissions & needed == needed);
                if allowed && !found.iter().any(|(c, _)| c.id == command.id) {
                    let app = set.apps.get(&command.application_id).cloned().unwrap_or_default();
                    found.push((command.clone(), app.into()));
                }
            }
        }
        found.sort_by_cached_key(|(c, _)| c.name.clone());
        autocomplete::rank(text, found, |(c, _)| vec![c.name.as_str()])
    }

    /// Enter with a picked command at the start of the box: runs it.
    /// Returns whether the text was a command, sent or refused.
    pub(super) fn run_typed_command(&mut self, text: &str, channel: Snowflake, cx: &mut Context<Self>) -> bool {
        // A command typed out in full runs too, as if picked.
        let typed = || {
            self.command_suggestions(text.trim_start().strip_prefix('/')?.split_whitespace().next()?)
                .into_iter()
                .map(|(command, _)| command)
                .find(|command| commands::arguments(command, text).is_some())
        };
        let Some(command) = self.composer.command.clone().or_else(typed) else { return false };
        let Some(args) = commands::arguments(&command, text) else {
            self.composer.command = None;
            return false;
        };
        match commands::interaction_data(&command, args) {
            Ok(data) => {
                let space = self.spaces.current();
                let guild = (!space.is_dms()).then_some(space.id);
                self.composer.command = None;
                self.composer.error = None;
                self.net.post(Command::Interact {
                    application: command.application_id,
                    guild,
                    channel,
                    data,
                    nonce: rest::nonce(),
                });
            }
            Err(error) => {
                self.composer.error = Some(
                    match error {
                        CommandError::Subcommand(names) => t!("commands.subcommand", names = names.join(", ")),
                        CommandError::Missing(name) => t!("commands.missing", name = name),
                        CommandError::Invalid(name) => t!("commands.invalid", name = name),
                        CommandError::Unsupported(name) => t!("commands.unsupported", name = name),
                    }
                    .into(),
                );
                // The box cleared itself on Enter; the command comes back.
                let text = text.to_owned();
                self.composer.input.update(cx, |input, cx| input.set_text(text, cx));
            }
        }
        cx.notify();
        true
    }

    /// The picked command's options, under the list's place, while it is
    /// being filled in.
    pub(super) fn render_command_hint(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let command = self.composer.command.as_ref()?;
        if self.autocomplete.is_open() || commands::arguments(command, self.composer.input.read(cx).text()).is_none() {
            return None;
        }
        let options = command.options.iter().enumerate().map(|(ix, option)| {
            let label = if option.required { format!("{}:", option.name) } else { format!("[{}:]", option.name) };
            div()
                .id(("command-option", ix))
                .when(!option.description.is_empty(), |chip| {
                    chip.tooltip(widgets::tooltip(option.description.clone()))
                })
                .px_1()
                .rounded_sm()
                .bg(theme::hover())
                .text_color(rgb(if option.required { theme::TEXT_STRONG } else { theme::TEXT_MUTED }))
                .child(label)
        });
        Some(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .px_3()
                .py_2()
                .rounded_t_lg()
                .bg(rgb(theme::BASE_LOW))
                .text_size(px(14.))
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(theme::TEXT_STRONG))
                        .child(format!("/{}", command.name)),
                )
                .children(options)
                .child(div().text_color(theme::muted()).truncate().child(command.description.clone()))
                .into_any_element(),
        )
    }
}
