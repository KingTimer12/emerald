//! The right-click menu of a server, category, channel or DM: mark as
//! read, mute for a while, what to be notified of, and the server's own
//! switches. Changes apply at once and go to Discord, which echoes them.

use gpui::{AnyElement, Context, FontWeight, SharedString, div, prelude::*, rgb};

use super::super::{icons, theme};
use super::Emerald;
use super::widgets::popover::Anchor;
use emerald_discord::model::{Notify, Snowflake};
use emerald_discord::{Command, MUTE_WINDOWS, SettingsChange};
use crate::zoom::zoomed;

/// What the menu is about.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Guild(Snowflake),
    /// A channel, category or DM; `guild` is `None` for DMs.
    Channel {
        guild: Option<Snowflake>,
        channel: Snowflake,
        category: bool,
        /// Threads keep their own settings on their membership, which this
        /// menu does not change yet; it only marks them read.
        thread: bool,
    },
}

/// Whether a channel of `kind` gets the header bell. Not threads: their
/// settings live on the thread membership, which Emerald does not write
/// yet, and the server's channel overrides are not where discord.com puts them.
pub(super) fn shows_bell(kind: emerald_discord::model::ChannelKind) -> bool {
    !kind.is_thread()
}

/// The header bell's glyph: slashed when the channel is muted or does not
/// notify of every message, as the web client draws it.
pub(super) fn bell_icon(muted: bool, level: Notify) -> &'static str {
    if muted || level != Notify::All { icons::BELL_OFF } else { icons::BELL }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Main,
    Mute,
    Notify,
}

pub struct PlaceMenu {
    target: Target,
    anchor: Anchor,
    page: Page,
}

impl Target {
    fn guild(self) -> Option<Snowflake> {
        match self {
            Self::Guild(guild) => Some(guild),
            Self::Channel { guild, .. } => guild,
        }
    }

    fn channel(self) -> Option<Snowflake> {
        match self {
            Self::Guild(_) => None,
            Self::Channel { channel, .. } => Some(channel),
        }
    }
}

impl Emerald {
    pub(super) fn open_place_menu(&mut self, target: Target, anchor: Anchor, cx: &mut Context<Self>) {
        self.place_menu = Some(PlaceMenu {
            target,
            anchor,
            page: Page::Main,
        });
        cx.notify();
    }

    /// The header bell's menu: this channel's notification levels.
    pub(super) fn open_notify_menu(&mut self, guild: Snowflake, channel: Snowflake, anchor: Anchor, cx: &mut Context<Self>) {
        let thread = self.spaces.channel(channel).is_some_and(|c| c.kind.is_thread());
        self.place_menu = Some(PlaceMenu {
            target: Target::Channel { guild: Some(guild), channel, category: false, thread },
            anchor,
            page: Page::Notify,
        });
        cx.notify();
    }

    /// Shift+click on the bell: mutes the channel until turned back on, or
    /// lifts its mute.
    pub(super) fn toggle_channel_mute(&mut self, guild: Snowflake, channel: Snowflake, muted: bool, cx: &mut Context<Self>) {
        let seconds = (!muted).then_some(-1);
        self.change_settings(Some(guild), SettingsChange::Mute { channel: Some(channel), seconds }, cx);
    }

    fn place_menu_page(&mut self, page: Page, cx: &mut Context<Self>) {
        if let Some(menu) = &mut self.place_menu {
            menu.page = page;
            cx.notify();
        }
    }

    /// Applies `change` here and sends it to Discord.
    pub(super) fn change_settings(&mut self, guild: Option<Snowflake>, change: SettingsChange, cx: &mut Context<Self>) {
        self.notify.apply(guild, change, crate::time::now_secs());
        self.net.post(Command::GuildSettings { guild, change });
        self.place_menu = None;
        cx.notify();
    }

    /// Folds or unfolds a category, remembered with the user's settings as
    /// on Discord.
    pub(super) fn toggle_category(&mut self, guild: Snowflake, category: Snowflake, cx: &mut Context<Self>) {
        let collapsed = !self.notify.is_collapsed(guild, category);
        self.change_settings(Some(guild), SettingsChange::Collapse { category, collapsed }, cx);
    }

    /// Marks `channel` read up to its newest message.
    pub(super) fn mark_channel_read(&mut self, channel: Snowflake, cx: &mut Context<Self>) {
        self.place_menu = None;
        if let Some(message) = self.newest_message(channel) {
            self.reads.acked(channel, message, 0);
            self.net.post(Command::Ack { channel, message });
        }
        cx.notify();
    }

    /// "Mark As Read" on a server: every unread channel in it, in one bulk
    /// receipt. The web client lists every channel; sending only the
    /// unread ones leaves the rest as they are, which is the same.
    pub(super) fn mark_guild_read(&mut self, guild: Snowflake, cx: &mut Context<Self>) {
        self.place_menu = None;
        let Some(space) = self.spaces.iter().find(|s| s.id == guild) else { return };
        let channels: Vec<(Snowflake, Snowflake)> = space
            .readable()
            .filter(|c| self.reads.is_unread(c.id) || self.reads.mentions(c.id) > 0)
            .filter_map(|c| Some((c.id, c.last_message_id?)))
            .collect();
        for (channel, message) in &channels {
            self.reads.acked(*channel, *message, 0);
        }
        if !channels.is_empty() {
            self.net.post(Command::AckBulk { channels });
        }
        cx.notify();
    }

    pub(super) fn render_place_menu(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let menu = self.place_menu.as_ref()?;
        let target = menu.target;
        let (guild, channel) = (target.guild(), target.channel());
        let mut items: Vec<AnyElement> = Vec::new();
        match menu.page {
            Page::Main => self.main_items(target, &mut items, cx),
            Page::Mute => {
                items.push(back("menu-back", cx));
                for (ix, seconds) in MUTE_WINDOWS.into_iter().enumerate() {
                    items.push(
                        item(("menu-mute", ix), mute_label(seconds), None)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let change = SettingsChange::Mute { channel, seconds: Some(seconds) };
                                this.change_settings(guild, change, cx)
                            }))
                            .into_any_element(),
                    );
                }
            }
            Page::Notify => {
                items.push(back("menu-back", cx));
                let current = self.notify.level(guild, channel);
                let mut levels = vec![Notify::All, Notify::Mentions, Notify::Nothing];
                if channel.is_some() {
                    levels.insert(0, Notify::Inherit);
                }
                for (ix, level) in levels.into_iter().enumerate() {
                    let checked = current == level || (channel.is_none() && current == Notify::Inherit && level == self.guild_default(guild));
                    items.push(
                        item(("menu-notify", ix), level_label(level), checked.then_some(icons::CHECK))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.change_settings(guild, SettingsChange::Notify { channel, level }, cx)
                            }))
                            .into_any_element(),
                    );
                }
            }
        }
        let list = super::widgets::popover::panel("place-menu", 240.)
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.place_menu = None;
                cx.notify();
            }))
            .children(items);
        Some(super::widgets::popover::at(menu.anchor, list))
    }

    /// The level a server notifies of when the user picked none.
    fn guild_default(&self, guild: Option<Snowflake>) -> Notify {
        self.notify.resolved(&super::state::notify::Place { guild, chain: Vec::new() })
    }

    fn main_items(&self, target: Target, items: &mut Vec<AnyElement>, cx: &mut Context<Self>) {
        let (guild, channel) = (target.guild(), target.channel());
        let now = crate::time::now_secs();
        let mute = self.notify.mute(guild, channel);
        let muted = mute.active(now);
        let separator = || div().my_1().h(zoomed(1.)).bg(theme::border()).into_any_element();
        match target {
            Target::Guild(guild) => items.push(
                item(("menu-read", 0), t!("read.mark_read").into(), None)
                    .on_click(cx.listener(move |this, _, _, cx| this.mark_guild_read(guild, cx)))
                    .into_any_element(),
            ),
            Target::Channel {
                channel, category: false, ..
            } => items.push(
                item(("menu-read", 0), t!("read.mark_read").into(), None)
                    .on_click(cx.listener(move |this, _, _, cx| this.mark_channel_read(channel, cx)))
                    .into_any_element(),
            ),
            Target::Channel { .. } => {}
        }
        self.manage_place_items(target, items, cx);
        if matches!(target, Target::Channel { thread: true, .. }) {
            return;
        }
        let more = |this: &Self, items: &mut Vec<AnyElement>, cx: &mut Context<Self>| this.more_place_items(target, items, cx);
        items.push(separator());
        let mute_label = match target {
            Target::Guild(_) => t!("mute.server"),
            Target::Channel { category: true, .. } => t!("mute.category"),
            Target::Channel { guild: None, .. } => t!("mute.conversation"),
            Target::Channel { .. } => t!("mute.channel"),
        };
        if muted {
            let until = mute.until.map(|end| SharedString::from(t!("mute.until", time = crate::time::header(end))));
            items.push(
                div()
                    .child(
                        item(("menu-unmute", 0), unmute_label(target), None).on_click(cx.listener(move |this, _, _, cx| {
                            this.change_settings(guild, SettingsChange::Mute { channel, seconds: None }, cx)
                        })),
                    )
                    .children(until.map(|until| div().px_2().pb_1().text_xs().text_color(theme::muted()).child(until)))
                    .into_any_element(),
            );
        } else {
            items.push(
                item(("menu-mute", 0), mute_label.into(), Some(icons::CHEVRON_RIGHT))
                    .on_click(cx.listener(|this, _, _, cx| this.place_menu_page(Page::Mute, cx)))
                    .into_any_element(),
            );
        }
        // Group DMs and DMs only mute; their level is always everything.
        if guild.is_some() {
            items.push(
                item(("menu-notify", 0), t!("notify.settings").into(), Some(icons::CHEVRON_RIGHT))
                    .on_click(cx.listener(|this, _, _, cx| this.place_menu_page(Page::Notify, cx)))
                    .into_any_element(),
            );
        }
        if let Target::Guild(guild) = target {
            let settings = self.notify.guild(Some(guild));
            let switches = [
                (t!("notify.suppress_everyone"), settings.is_some_and(|s| s.suppress_everyone), SettingsChange::SuppressEveryone as fn(bool) -> SettingsChange),
                (t!("notify.suppress_roles"), settings.is_some_and(|s| s.suppress_roles), SettingsChange::SuppressRoles),
                (t!("notify.hide_muted"), settings.is_some_and(|s| s.hide_muted_channels), SettingsChange::HideMuted),
            ];
            items.push(separator());
            for (ix, (label, on, change)) in switches.into_iter().enumerate() {
                items.push(
                    item(("menu-switch", ix), label.into(), on.then_some(icons::CHECK))
                        .on_click(cx.listener(move |this, _, _, cx| this.change_settings(Some(guild), change(!on), cx)))
                        .into_any_element(),
                );
            }
        }
        more(self, items, cx);
    }
}

/// A menu row with a second, muted line under its title, such as a setting
/// and its current value.
pub(super) fn item_two_line(
    id: (&'static str, usize),
    title: SharedString,
    subtitle: SharedString,
    trailing: Option<&'static str>,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .py_1p5()
        .px_2()
        .flex()
        .items_center()
        .gap_2()
        .rounded(theme::radius::control())
        .cursor_pointer()
        .hover(|s| s.bg(theme::hover()))
        .child(
            div()
                .flex_1()
                .min_w(zoomed(0.))
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(zoomed(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(theme::text_default()))
                        .child(title),
                )
                .child(div().truncate().text_size(zoomed(12.)).text_color(theme::muted()).child(subtitle)),
        )
        .children(trailing.map(|glyph| icons::icon(glyph, 16., theme::interactive())))
}

pub(super) fn item(id: (&'static str, usize), label: SharedString, trailing: Option<&'static str>) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h(zoomed(32.))
        .px_2()
        .flex()
        .items_center()
        .gap_2()
        .rounded(theme::radius::control())
        .cursor_pointer()
        .text_size(zoomed(14.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(theme::text_default()))
        .hover(|s| s.bg(theme::hover()))
        .child(div().flex_1().child(label))
        .children(trailing.map(|glyph| icons::icon(glyph, 16., theme::interactive())))
}

fn back(id: &'static str, cx: &mut Context<Emerald>) -> AnyElement {
    item((id, 0), t!("common.back").into(), None)
        .on_click(cx.listener(|this, _, _, cx| this.place_menu_page(Page::Main, cx)))
        .into_any_element()
}

fn unmute_label(target: Target) -> SharedString {
    match target {
        Target::Guild(_) => t!("mute.unmute_server"),
        Target::Channel { category: true, .. } => t!("mute.unmute_category"),
        Target::Channel { guild: None, .. } => t!("mute.unmute_conversation"),
        Target::Channel { .. } => t!("mute.unmute_channel"),
    }
    .into()
}

fn mute_label(seconds: i64) -> SharedString {
    match seconds {
        900 => t!("mute.minutes_15"),
        3600 => t!("mute.hour_1"),
        10_800 => t!("mute.hours_3"),
        28_800 => t!("mute.hours_8"),
        86_400 => t!("mute.hours_24"),
        _ => t!("mute.forever"),
    }
    .into()
}

fn level_label(level: Notify) -> SharedString {
    match level {
        Notify::All => t!("notify.all"),
        Notify::Mentions => t!("notify.mentions"),
        Notify::Nothing => t!("notify.nothing"),
        Notify::Inherit => t!("notify.default"),
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threads_get_no_bell() {
        use emerald_discord::model::ChannelKind;
        assert!(shows_bell(ChannelKind::GUILD_TEXT));
        assert!(!shows_bell(ChannelKind::PUBLIC_THREAD));
        assert!(!shows_bell(ChannelKind::PRIVATE_THREAD));
    }

    #[test]
    fn bell_is_slashed_when_muted_or_not_every_message() {
        assert_eq!(bell_icon(false, Notify::All), icons::BELL);
        assert_eq!(bell_icon(true, Notify::All), icons::BELL_OFF);
        assert_eq!(bell_icon(false, Notify::Mentions), icons::BELL_OFF);
        assert_eq!(bell_icon(false, Notify::Nothing), icons::BELL_OFF);
    }
}
