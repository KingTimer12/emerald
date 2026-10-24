//! The suggestion list over the message box: members, roles, channels and
//! emoji for the word being typed, picked with the arrows and Tab or Enter,
//! or the mouse. [`state::autocomplete`] decides what is being completed;
//! this builds the entries and draws them.

use std::time::{Duration, Instant};

use gpui::{AnyElement, Context, FontWeight, SharedString, div, prelude::*, px, relative, rgb};

use super::super::text_input::MenuKey;
use super::super::{emoji, icons, theme};
use super::state::autocomplete::{self, Completion, Query, Trigger};
use super::{Emerald, widgets};
use crate::discord::model::{AppCommand, ChannelKind, Emoji, Snowflake};
use crate::discord::{Command, permissions};

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

    fn build_suggestions(&self, query: &Query) -> Vec<Suggestion> {
        match query.trigger {
            Trigger::Mention => self.mention_suggestions(&query.text),
            Trigger::Channel => self.channel_suggestions(&query.text),
            Trigger::Emoji => self.emoji_suggestions(&query.text),
            Trigger::Command => self
                .command_suggestions(&query.text)
                .into_iter()
                .map(|(command, app)| Suggestion::Command { command: Box::new(command), app })
                .collect(),
        }
    }

    /// Members of the guild seen so far, or the people in a DM; then the
    /// roles one may ping, and `@everyone` and `@here` when allowed.
    fn mention_suggestions(&self, text: &str) -> Vec<Suggestion> {
        let space = self.spaces.current();
        let Some(channel) = self.selected_channel else { return Vec::new() };
        let mut people: Vec<(Snowflake, String, String, Option<u32>)> = match space.info() {
            Some(guild) => guild
                .members()
                .filter_map(|(id, member)| {
                    let user = member.user.as_ref().or_else(|| self.people.user(id))?;
                    let name = member
                        .nick
                        .clone()
                        .or_else(|| user.global_name.clone())
                        .unwrap_or_else(|| user.username.clone());
                    Some((id, name, user.username.clone(), guild.role_color(&member.roles)))
                })
                .collect(),
            None => {
                let recipients = space.channels().iter().find(|c| c.id == channel).map(|c| c.recipients.as_slice());
                recipients
                    .unwrap_or_default()
                    .iter()
                    .chain(self.people.me())
                    .map(|user| {
                        let name = user.global_name.clone().unwrap_or_else(|| user.username.clone());
                        (user.id, name, user.username.clone(), None)
                    })
                    .collect()
            }
        };
        // A steady order for equal matches: by name.
        people.sort_by_cached_key(|(_, name, _, _)| name.to_lowercase());
        let mut found: Vec<Suggestion> = autocomplete::rank(text, people, |(_, name, username, _)| {
            vec![name.as_str(), username.as_str()]
        })
        .into_iter()
        .map(|(id, name, username, color)| Suggestion::Member {
            id,
            name: name.into(),
            username: username.into(),
            color,
        })
        .collect();

        let Some(guild) = space.info() else { return found };
        let everyone = space.permissions(channel) & permissions::MENTION_EVERYONE != 0;
        let mut roles: Vec<_> = guild
            .roles()
            .iter()
            // The @everyone role shares the guild's id and is offered below.
            .filter(|role| role.id != space.id && (role.mentionable || everyone))
            .collect();
        roles.sort_by_key(|role| std::cmp::Reverse(role.position));
        found.extend(
            autocomplete::rank(text, roles, |role| vec![role.name.as_str()])
                .into_iter()
                .take(ROLES)
                .map(|role| Suggestion::Role {
                    id: role.id,
                    name: role.name.clone().into(),
                    color: (role.color != 0).then_some(role.color),
                }),
        );
        if everyone {
            let text = text.to_lowercase();
            found.extend(["everyone", "here"].into_iter().filter(|w| w.starts_with(&text)).map(Suggestion::Everyone));
        }
        found
    }

    /// The guild's channels one can see, threads left out.
    fn channel_suggestions(&self, text: &str) -> Vec<Suggestion> {
        let space = self.spaces.current();
        if space.is_dms() {
            return Vec::new();
        }
        let channels = space.channels().iter().filter(|c| {
            !c.kind.is_thread()
                && c.kind != ChannelKind::CATEGORY
                && space.permissions(c.id) & permissions::VIEW_CHANNEL != 0
        });
        autocomplete::rank(text, channels, |c| vec![c.name.as_deref().unwrap_or_default()])
            .into_iter()
            .map(|c| Suggestion::Channel {
                id: c.id,
                name: c.name.clone().unwrap_or_default().into(),
                icon: space.row(c.id).map_or(icons::TEXT, |row| row.icon),
            })
            .collect()
    }

    /// Custom emoji one may use here, then Unicode ones by shortcode.
    fn emoji_suggestions(&self, text: &str) -> Vec<Suggestion> {
        let custom = self.custom_emoji().into_iter().flat_map(|(_, emojis)| emojis).filter_map(|emoji| {
            let name = emoji.name.clone()?;
            Some((emoji, name))
        });
        let unicode = emoji::by_category().iter().flat_map(|(_, list)| list).map(|e| {
            let name = emoji::name(e.as_str()).unwrap_or(e.name()).to_owned();
            (Emoji::unicode(e.as_str()), name)
        });
        let text = text.to_lowercase();
        autocomplete::rank(&text, custom.chain(unicode), |(emoji, name)| {
            let mut names = vec![name.as_str()];
            if emoji.id.is_none()
                && let Some(e) = emoji.name.as_deref().and_then(emojis::get)
            {
                names.extend(e.shortcodes());
            }
            names
        })
        .into_iter()
        .map(|(emoji, name)| Suggestion::Emoji { emoji, name: name.into() })
        .collect()
    }

    /// The list, above the message box.
    pub(super) fn render_suggestions(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.autocomplete.is_open() {
            return None;
        }
        let selected = self.autocomplete.selected();
        let mut rows: Vec<AnyElement> = Vec::new();
        let mut section = None;
        for (ix, suggestion) in self.suggestions.iter().enumerate() {
            if section != Some(suggestion.section()) {
                section = Some(suggestion.section());
                rows.push(
                    div()
                        .px_2()
                        .pt_2()
                        .pb_1()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::muted())
                        .child(suggestion.section())
                        .into_any_element(),
                );
            }
            rows.push(self.render_suggestion(ix, suggestion, ix == selected, cx));
        }
        Some(
            div()
                .id("suggestions")
                .absolute()
                .left_0()
                .right_0()
                .bottom(relative(1.))
                .mb_2()
                .max_h(px(ROW * 10. + 64.))
                .overflow_y_scroll()
                .p_2()
                .flex()
                .flex_col()
                .rounded_lg()
                .border_1()
                .border_color(theme::border())
                .bg(rgb(theme::SURFACE_HIGH))
                .shadow_lg()
                .occlude()
                .children(rows)
                .into_any_element(),
        )
    }

    fn render_suggestion(&self, ix: usize, suggestion: &Suggestion, selected: bool, cx: &mut Context<Self>) -> AnyElement {
        let (leading, name, detail, color): (AnyElement, SharedString, Option<SharedString>, u32) = match suggestion {
            Suggestion::Member { id, name, username, color } => (
                widgets::avatar(name, *id, self.user_avatar(*id), 24., None).into_any_element(),
                name.clone(),
                Some(username.clone()),
                color.unwrap_or(theme::TEXT_STRONG),
            ),
            Suggestion::Role { name, color, .. } => (
                div().w(px(24.)).flex().justify_center().child("@").into_any_element(),
                format!("@{name}").into(),
                None,
                color.unwrap_or(theme::TEXT_STRONG),
            ),
            Suggestion::Everyone(word) => (
                div().w(px(24.)).flex().justify_center().child("@").into_any_element(),
                format!("@{word}").into(),
                Some(if *word == "here" { t!("autocomplete.here") } else { t!("autocomplete.everyone") }.into()),
                theme::TEXT_STRONG,
            ),
            Suggestion::Channel { name, icon, .. } => (
                icons::icon(icon, 20., theme::CHANNELS_DEFAULT).into_any_element(),
                name.clone(),
                None,
                theme::TEXT_STRONG,
            ),
            Suggestion::Emoji { emoji, name } => (
                self.emoji_image(emoji, 24.),
                format!(":{name}:").into(),
                None,
                theme::TEXT_STRONG,
            ),
            Suggestion::Command { command, app } => (
                div().w(px(24.)).flex().justify_center().child("/").into_any_element(),
                format!("/{}", command.name).into(),
                Some(if command.description.is_empty() {
                    app.clone()
                } else {
                    format!("{} · {app}", command.description).into()
                }),
                theme::TEXT_STRONG,
            ),
        };
        div()
            .id(("suggestion", ix))
            .h(px(ROW))
            .flex_none()
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .rounded_md()
            .cursor_pointer()
            .when(selected, |row| row.bg(theme::selected()))
            .child(div().size(px(24.)).flex_none().flex().items_center().justify_center().child(leading))
            .child(
                div()
                    .flex_none()
                    .max_w(relative(0.6))
                    .truncate()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(color))
                    .child(name),
            )
            .children(detail.map(|detail| {
                div().flex_1().min_w(px(0.)).truncate().text_size(px(14.)).text_color(theme::muted()).child(detail)
            }))
            .on_hover(cx.listener(move |this, inside: &bool, _, cx| {
                if *inside && this.autocomplete.selected() != ix {
                    this.autocomplete.select(ix);
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |this, _, _, cx| this.accept_suggestion(ix, cx)))
            .into_any_element()
    }
}
