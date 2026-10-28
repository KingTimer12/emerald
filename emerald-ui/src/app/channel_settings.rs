//! Channel settings: name, topic, slow mode and, in forums, the tags posts
//! can carry. The dialog saves the whole set at once.

use gpui::{AnyElement, Context, Entity, Focusable, SharedString, Window, div, prelude::*, rgb};

use super::dialogs::dialog;
use super::place_menu::{Target, item};
use super::widgets::{self, ButtonKind};
use super::{Emerald, Overlay};
use crate::text_input::TextInput;
use crate::theme;
use crate::time;
use crate::zoom::zoomed;
use emerald_discord::model::Snowflake;
use emerald_discord::permissions::{MANAGE_CHANNELS, MANAGE_THREADS};
use emerald_discord::{ChannelEdit, Command, TagEdit};

/// Slow mode choices in seconds, as the web client offers them.
const SLOW_MODES: [u32; 14] = [0, 5, 10, 15, 30, 60, 120, 300, 600, 900, 1_800, 3_600, 7_200, 21_600];
/// Discord keeps at most this many tags on a forum.
const TAG_LIMIT: usize = 20;

/// The dialog's state. Fields hold what the user has typed so far.
pub(super) struct ChannelSettingsModal {
    pub(super) channel: Snowflake,
    /// Tags are only shown and saved for forums.
    forum: bool,
    name: Entity<TextInput>,
    topic: Entity<TextInput>,
    tag_input: Entity<TextInput>,
    slow_mode: u32,
    /// The tags as they will be saved. Tags that came from the channel keep
    /// their id and emoji, so saving does not drop them.
    tags: Vec<TagEdit>,
    pub(super) error: Option<SharedString>,
}

impl Emerald {
    /// Opens the settings of `channel` with its current values.
    pub(super) fn open_channel_settings(&mut self, channel: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        let Some(current) = self.spaces.channel(channel) else { return };
        let name = current.name.clone().unwrap_or_default();
        let topic = current.topic.clone().unwrap_or_default();
        let slow_mode = current.rate_limit_per_user.unwrap_or(0);
        let forum = current.kind.is_forum();
        let tags = current
            .available_tags
            .iter()
            .map(|tag| TagEdit {
                id: Some(tag.id),
                name: tag.name.clone(),
                moderated: tag.moderated,
                emoji_id: tag.emoji_id,
                emoji_name: tag.emoji_name.clone(),
            })
            .collect();
        let name_input = cx.new(|cx| TextInput::new(t!("channel_settings.name"), cx).keep_on_submit().submit_empty());
        let topic_input = cx.new(|cx| TextInput::new(t!("channel_settings.topic"), cx).keep_on_submit().submit_empty());
        let tag_input = cx.new(|cx| TextInput::new(t!("channel_settings.tag_placeholder"), cx).keep_on_submit().submit_empty());
        name_input.update(cx, |input, cx| input.set_text(name, cx));
        topic_input.update(cx, |input, cx| input.set_text(topic, cx));
        window.focus(&name_input.read(cx).focus_handle(cx));
        self.place_menu = None;
        self.overlay = Some(Overlay::ChannelSettings(Box::new(ChannelSettingsModal {
            channel,
            forum,
            name: name_input,
            topic: topic_input,
            tag_input,
            slow_mode,
            tags,
            error: None,
        })));
        cx.notify();
    }

    pub(super) fn set_slow_mode(&mut self, seconds: u32, cx: &mut Context<Self>) {
        if let Some(Overlay::ChannelSettings(modal)) = &mut self.overlay {
            modal.slow_mode = seconds;
            cx.notify();
        }
    }

    /// Adds the typed tag, unless the forum is full or already has it.
    pub(super) fn add_channel_tag(&mut self, cx: &mut Context<Self>) {
        let Some(Overlay::ChannelSettings(modal)) = &mut self.overlay else { return };
        let name = modal.tag_input.read(cx).text().trim().to_owned();
        if name.is_empty() {
            return;
        }
        if modal.tags.len() >= TAG_LIMIT {
            modal.error = Some(t!("channel_settings.tags_full").into());
        } else if modal.tags.iter().any(|tag| tag.name.eq_ignore_ascii_case(&name)) {
            modal.error = Some(t!("channel_settings.tag_exists").into());
        } else {
            modal.tags.push(TagEdit { id: None, name, moderated: false, emoji_id: None, emoji_name: None });
            modal.error = None;
            modal.tag_input.update(cx, |input, cx| input.set_text("", cx));
        }
        cx.notify();
    }

    pub(super) fn remove_channel_tag(&mut self, ix: usize, cx: &mut Context<Self>) {
        if let Some(Overlay::ChannelSettings(modal)) = &mut self.overlay {
            if ix < modal.tags.len() {
                modal.tags.remove(ix);
            }
            cx.notify();
        }
    }

    /// Sends the whole dialog. The name is required; the other fields may be
    /// cleared.
    pub(super) fn save_channel_settings(&mut self, cx: &mut Context<Self>) {
        let Some(Overlay::ChannelSettings(modal)) = &mut self.overlay else { return };
        let name = modal.name.read(cx).text().trim().to_owned();
        if name.is_empty() {
            modal.error = Some(t!("channel_settings.name_required").into());
            cx.notify();
            return;
        }
        let edit = ChannelEdit {
            name: Some(name),
            topic: Some(modal.topic.read(cx).text().trim().to_owned()),
            slow_mode: Some(modal.slow_mode),
            tags: modal.forum.then(|| modal.tags.clone()),
            ..Default::default()
        };
        modal.error = None;
        let channel = modal.channel;
        self.net.post(Command::EditChannel { channel, edit });
        cx.notify();
    }

    pub(super) fn render_channel_settings(&self, modal: &ChannelSettingsModal, cx: &mut Context<Self>) -> AnyElement {
        let slow_modes = SLOW_MODES.into_iter().enumerate().map(|(ix, seconds)| {
            let label = if seconds == 0 { t!("channel_settings.slow_off").to_owned() } else { time::duration(seconds.into()) };
            widgets::chip(("slow-mode", ix), label, seconds == modal.slow_mode)
                .on_click(cx.listener(move |this, _, _, cx| this.set_slow_mode(seconds, cx)))
        });
        let mut content = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(widgets::label(t!("channel_settings.name")))
            .child(modal.name.clone())
            .child(widgets::label(t!("channel_settings.topic")))
            .child(modal.topic.clone())
            .child(widgets::label(t!("channel_settings.slow_mode")))
            .child(div().flex().flex_wrap().gap_2().children(slow_modes));
        if modal.forum {
            let tags = modal.tags.iter().enumerate().map(|(ix, tag)| {
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().text_color(rgb(theme::text_default())).child(tag.name.clone()))
                    .child(
                        widgets::button(("tag-remove", ix), t!("channel_settings.tag_remove"), ButtonKind::Secondary)
                            .on_click(cx.listener(move |this, _, _, cx| this.remove_channel_tag(ix, cx))),
                    )
            });
            content = content
                .child(widgets::label(t!("channel_settings.tags")))
                .child(div().flex().flex_col().gap_2().children(tags))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(div().flex_1().child(modal.tag_input.clone()))
                        .child(
                            widgets::button("tag-add", t!("channel_settings.tag_add"), ButtonKind::Secondary)
                                .on_click(cx.listener(|this, _, _, cx| this.add_channel_tag(cx))),
                        ),
                );
        }
        if let Some(error) = &modal.error {
            content = content.child(div().text_size(zoomed(14.)).text_color(rgb(theme::feedback_critical())).child(error.clone()));
        }
        let save = widgets::button("channel-save", t!("common.save"), ButtonKind::Brand)
            .on_click(cx.listener(|this, _, _, cx| this.save_channel_settings(cx)));
        dialog(t!("channel_settings.title").into(), content.into_any_element(), save, cx)
    }
}

impl Emerald {
    /// The place menu's moderation items: channel settings for those who
    /// manage channels, lock and archive for those who manage threads. Only
    /// in the server on screen, since permissions are read from it.
    pub(super) fn manage_place_items(&self, target: Target, items: &mut Vec<AnyElement>, cx: &mut Context<Self>) {
        let Target::Channel { guild: Some(guild), channel, category: false, thread } = target else { return };
        let space = self.spaces.current();
        if space.id != guild {
            return;
        }
        let granted = space.permissions(channel);
        let mut own = Vec::new();
        if thread {
            if granted & MANAGE_THREADS == 0 {
                return;
            }
            let Some(current) = self.spaces.channel(channel) else { return };
            let locked = current.is_locked();
            let archived = current.thread_metadata.as_ref().is_some_and(|metadata| metadata.archived);
            let lock = if locked { t!("channel_settings.unlock_thread") } else { t!("channel_settings.lock_thread") };
            let archive = if archived { t!("channel_settings.unarchive_thread") } else { t!("channel_settings.archive_thread") };
            own.push(
                item(("menu-lock-thread", 0), lock.into(), None)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let edit = ChannelEdit { locked: Some(!locked), ..Default::default() };
                        this.edit_channel_from_menu(channel, edit, cx)
                    }))
                    .into_any_element(),
            );
            own.push(
                item(("menu-archive-thread", 0), archive.into(), None)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let edit = ChannelEdit { archived: Some(!archived), ..Default::default() };
                        this.edit_channel_from_menu(channel, edit, cx)
                    }))
                    .into_any_element(),
            );
        } else if granted & MANAGE_CHANNELS != 0 {
            own.push(
                item(("menu-channel-settings", 0), t!("channel_settings.open").into(), None)
                    .on_click(cx.listener(move |this, _, window, cx| this.open_channel_settings(channel, window, cx)))
                    .into_any_element(),
            );
        }
        if !own.is_empty() {
            items.push(div().my_1().h(zoomed(1.)).bg(theme::border()).into_any_element());
            items.extend(own);
        }
    }

    /// Saves a thread's lock or archive state from the place menu.
    fn edit_channel_from_menu(&mut self, channel: Snowflake, edit: ChannelEdit, cx: &mut Context<Self>) {
        self.place_menu = None;
        self.net.post(Command::EditChannel { channel, edit });
        cx.notify();
    }
}
