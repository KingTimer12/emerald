//! Starting threads and forum posts, and joining or leaving a thread.
//! "Create Thread" names the thread after its message as the web client
//! does; a post takes a title, tags and its first message.

use crate::zoom::zoomed;

mod forms;

use gpui::{
    AnyElement, Context, Entity, Focusable, FontWeight, MouseButton, SharedString, Subscription, Window, div,
    prelude::*, rgb,
};

use super::super::super::text_input::{PastedImage, Submitted, TextInput};
use super::super::super::{theme, time};
use super::super::state::Msg;
use super::super::{Emerald, Overlay, widgets};
use emerald_discord::model::{
    Channel, ChannelKind, DEFAULT_AUTO_ARCHIVE, Snowflake, ThreadMember, message_kind,
};
use emerald_discord::{Action, Command, permissions};

/// Discord's limits on a thread's name and a post's tags.
const MAX_NAME: usize = 100;
const MAX_TAGS: usize = 5;
/// The web client cuts names it makes up from a message at this length.
const DEFAULT_NAME_LEN: usize = 40;
/// Rows the post's message grows to before it scrolls.
const POST_ROWS: usize = 6;

/// "Create Thread", from `message` in `parent` when given.
pub(in super::super) struct NewThread {
    parent: Snowflake,
    message: Option<Snowflake>,
    name: Entity<TextInput>,
    pub error: Option<SharedString>,
    _subscription: Subscription,
}

/// "New Post" in `forum`.
pub(in super::super) struct NewPost {
    forum: Snowflake,
    title: Entity<TextInput>,
    content: Entity<TextInput>,
    tags: Vec<Snowflake>,
    pub error: Option<SharedString>,
    _subscriptions: [Subscription; 3],
}

impl Emerald {
    /// A message can start a thread in a guild's text or announcement
    /// channel, once, for those allowed to make public threads.
    pub(super) fn may_start_thread(&self, channel: Snowflake, msg: &Msg) -> bool {
        let space = self.spaces.current();
        let kind = space.channels().iter().find(|c| c.id == channel).map(|c| c.kind);
        !space.is_dms()
            && matches!(kind, Some(ChannelKind::GUILD_TEXT | ChannelKind::ANNOUNCEMENT))
            && msg.thread.is_none()
            && !msg.pending
            && matches!(msg.kind, message_kind::DEFAULT | message_kind::REPLY)
            && space.permissions(channel) & permissions::CREATE_PUBLIC_THREADS != 0
    }

    pub(super) fn open_create_thread(
        &mut self,
        parent: Snowflake,
        message: Option<Snowflake>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.message_menu = None;
        let suggested = message
            .and_then(|m| self.history.find(parent, m))
            .map(|(_, msg)| default_thread_name(&msg.content))
            .unwrap_or_default();
        let name = cx.new(|cx| {
            let mut input = TextInput::new(t!("thread.name_placeholder"), cx).keep_on_submit().submit_empty();
            input.set_text(suggested, cx);
            input
        });
        let subscription = cx.subscribe(&name, |this, _, _: &Submitted, cx| this.create_thread(cx));
        window.focus(&name.focus_handle(cx));
        self.overlay = Some(Overlay::CreateThread(NewThread {
            parent,
            message,
            name,
            error: None,
            _subscription: subscription,
        }));
        cx.notify();
    }

    fn create_thread(&mut self, cx: &mut Context<Self>) {
        let Some(Overlay::CreateThread(modal)) = &mut self.overlay else { return };
        let mut name: String = modal.name.read(cx).text().trim().chars().take(MAX_NAME).collect();
        if name.is_empty() {
            name = t!("thread.default_name").to_owned();
        }
        modal.error = None;
        let (parent, message) = (modal.parent, modal.message);
        let parent_channel = self.spaces.channel(parent);
        let kind = if parent_channel.is_some_and(|c| c.kind == ChannelKind::ANNOUNCEMENT) {
            ChannelKind::ANNOUNCEMENT_THREAD
        } else {
            ChannelKind::PUBLIC_THREAD
        };
        let auto_archive = auto_archive(parent_channel);
        let command = Command::CreateThread {
            parent,
            message,
            name,
            kind,
            auto_archive,
        };
        self.run(Action::CreateThread, command, cx);
    }

    pub(super) fn open_create_post(&mut self, forum: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        let title = cx.new(|cx| TextInput::new(t!("forum.title_placeholder"), cx).keep_on_submit());
        let content = cx.new(|cx| TextInput::new(t!("forum.content_placeholder"), cx).keep_on_submit().multiline(POST_ROWS));
        let next = content.focus_handle(cx);
        let subscriptions = [
            // Enter in the title moves on to the message.
            cx.subscribe_in(&title, window, move |_, _, _: &Submitted, window, _| window.focus(&next)),
            cx.subscribe(&content, |this, _, _: &Submitted, cx| this.create_post(cx)),
            cx.subscribe(&content, |this, _, event: &PastedImage, cx| this.attach_pasted(&event.0, cx)),
        ];
        // The post's files wait where the message box keeps its own; the
        // box is hidden while a forum is open.
        self.composer.attachments.clear();
        window.focus(&title.focus_handle(cx));
        self.overlay = Some(Overlay::CreatePost(NewPost {
            forum,
            title,
            content,
            tags: Vec::new(),
            error: None,
            _subscriptions: subscriptions,
        }));
        cx.notify();
    }

    fn toggle_post_tag(&mut self, tag: Snowflake, cx: &mut Context<Self>) {
        let Some(Overlay::CreatePost(modal)) = &mut self.overlay else { return };
        if let Some(ix) = modal.tags.iter().position(|&t| t == tag) {
            modal.tags.remove(ix);
        } else if modal.tags.len() < MAX_TAGS {
            modal.tags.push(tag);
        }
        cx.notify();
    }

    fn create_post(&mut self, cx: &mut Context<Self>) {
        let Some(Overlay::CreatePost(modal)) = &mut self.overlay else { return };
        let forum = self.spaces.channel(modal.forum);
        let name: String = modal.title.read(cx).text().trim().chars().take(MAX_NAME).collect();
        let content = modal.content.read(cx).text().trim().to_owned();
        let has_files = !self.composer.attachments.is_empty();
        let media = forum.is_some_and(|f| f.kind == ChannelKind::MEDIA);
        let error = if name.is_empty() {
            Some(t!("forum.title_required"))
        } else if media && !has_files {
            Some(t!("forum.files_required"))
        } else if content.is_empty() && !has_files {
            Some(t!("forum.content_required"))
        } else if modal.tags.is_empty() && forum.is_some_and(Channel::requires_tag) {
            Some(t!("forum.tag_required"))
        } else {
            None
        };
        modal.error = error.map(Into::into);
        if error.is_some() {
            cx.notify();
            return;
        }
        let command = Command::CreatePost {
            forum: modal.forum,
            name,
            tags: modal.tags.clone(),
            content,
            auto_archive: auto_archive(forum),
            // Kept in the form until the post exists, in case it fails.
            files: self.composer.attachments.iter().map(|a| a.upload.clone()).collect(),
        };
        self.run(Action::CreateThread, command, cx);
    }

    /// The thread or post we started: we are in it, and it opens.
    pub(in super::super) fn on_thread_created(&mut self, mut thread: Channel, window: &mut Window, cx: &mut Context<Self>) {
        self.busy.remove(&Action::CreateThread);
        if matches!(self.overlay, Some(Overlay::CreatePost(_))) {
            self.composer.attachments.clear();
        }
        if matches!(self.overlay, Some(Overlay::CreateThread(_) | Overlay::CreatePost(_))) {
            self.close_overlay(window, cx);
        }
        let Some(guild) = thread.guild_id else { return };
        if thread.member.is_none() {
            thread.member = Some(joined_now());
        }
        let id = thread.id;
        if let Some(space) = self.spaces.get_mut(guild) {
            space.upsert_channel(thread);
        }
        if self.spaces.current().id == guild {
            self.select_channel(id, window, cx);
        }
    }

    /// A join or leave went through; the gateway may say so later too.
    pub(in super::super) fn on_thread_membership(&mut self, thread: Snowflake, joined: bool) {
        self.busy.remove(&Action::ThreadMembership);
        let Some(channel) = self.spaces.channel(thread) else { return };
        // A gateway echo that came first holds the real join time.
        if joined == channel.member.is_some() {
            return;
        }
        let Some(guild) = channel.guild_id else { return };
        if let Some(space) = self.spaces.get_mut(guild) {
            space.set_thread_member(thread, joined.then(joined_now));
        }
    }

    pub(in super::super) fn creating_thread_failed(&mut self, error: String) {
        match &mut self.overlay {
            Some(Overlay::CreateThread(modal)) => modal.error = Some(error.into()),
            Some(Overlay::CreatePost(modal)) => modal.error = Some(error.into()),
            _ => self.composer.error = Some(error.into()),
        }
    }

    /// "Join Thread" or "Leave Thread" in the open thread's header.
    pub(super) fn render_membership_button(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let id = self.selected_channel?;
        let thread = self.spaces.current().channels().iter().find(|c| c.id == id && c.kind.is_thread())?;
        let joined = thread.member.is_some();
        // Private threads are joined by invitation only.
        if thread.kind == ChannelKind::PRIVATE_THREAD && !joined {
            return None;
        }
        let (label, kind) = if joined {
            (t!("thread.leave"), widgets::ButtonKind::Secondary)
        } else {
            (t!("thread.join"), widgets::ButtonKind::Brand)
        };
        let busy = self.busy.contains(&Action::ThreadMembership);
        let forum_post = thread.parent_id.and_then(|p| self.spaces.channel(p)).is_some_and(|p| p.kind.is_forum());
        Some(
            widgets::button("thread-membership", label, kind)
                .h(zoomed(32.))
                .when(busy, |button| button.opacity(0.5))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.run(Action::ThreadMembership, Command::ThreadMembership { thread: id, join: !joined, forum_post }, cx);
                }))
                .into_any_element(),
        )
    }
}

/// How long a new thread in `parent` stays active without messages.
fn auto_archive(parent: Option<&Channel>) -> u32 {
    parent.and_then(|c| c.default_auto_archive_duration).unwrap_or(DEFAULT_AUTO_ARCHIVE)
}

/// Our membership as of now, until Discord tells the real join time.
fn joined_now() -> ThreadMember {
    ThreadMember {
        join_timestamp: Some(time::iso_utc(time::now_secs())),
    }
}

/// The name the web client suggests for a thread on a message: its first
/// line without markdown, cut at a word boundary within 40 characters.
fn default_thread_name(content: &str) -> String {
    let line = content.lines().next().unwrap_or_default();
    let plain: String = line.chars().filter(|c| !matches!(c, '*' | '_' | '~' | '`' | '|')).collect();
    let plain = plain.trim_start_matches([' ', '#', '-', '>']);
    let separator = |c: char| c.is_whitespace() || "!@#$%^&*()_-+={}[]:\";'<>?,./".contains(c);
    // Whole words while they fit, each with the separators before it.
    let mut name = String::new();
    let mut rest = plain;
    while !rest.is_empty() {
        let word_end = rest.find(separator).unwrap_or(rest.len());
        let gap_end = rest[word_end..].find(|c| !separator(c)).map_or(rest.len(), |i| word_end + i);
        let (word, gap) = (&rest[..word_end], &rest[word_end..gap_end]);
        if !name.is_empty() && name.chars().count() + word.chars().count() > DEFAULT_NAME_LEN {
            break;
        }
        name.push_str(word);
        if name.chars().count() + gap.chars().count() > DEFAULT_NAME_LEN {
            break;
        }
        name.push_str(gap);
        rest = &rest[gap_end..];
    }
    name.chars().take(DEFAULT_NAME_LEN).collect::<String>().trim_end().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_the_message() {
        assert_eq!(default_thread_name("Hello **there**\nsecond line"), "Hello there");
        assert_eq!(default_thread_name("# Title here"), "Title here");
        assert_eq!(
            default_thread_name("one two three four five six seven eight nine ten"),
            "one two three four five six seven eight"
        );
        assert_eq!(default_thread_name(&"a".repeat(60)), "a".repeat(40));
        assert_eq!(default_thread_name(""), "");
    }
}
