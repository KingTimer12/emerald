//! The message box under the chat: sending, the typing signal, errors, and
//! the draft each channel keeps while the user is elsewhere.

use gpui::{AppContext, Context, Entity, SharedString};

use super::attachments::Attachment;
use super::chat::Replying;
use super::state::autocomplete::{self, Completion};
use super::state::{Drafts, Msg};
use super::{Emerald, shown};
use super::super::text_input::TextInput;
use crate::discord::model::{AppCommand, Snowflake};
use crate::discord::{Command, ReplyTo, Upload, rest};

const MAX_MESSAGE_LEN: usize = 2000;
/// Rows the message box grows to before it scrolls.
pub(super) const MAX_ROWS: usize = 12;

pub(super) struct Composer {
    pub input: Entity<TextInput>,
    /// Draft text when the input last changed, to tell edits from cursor moves.
    draft: SharedString,
    /// Why the last message in the open channel was not sent.
    pub error: Option<SharedString>,
    /// The message the next send answers.
    pub reply: Option<Replying>,
    /// Files the next send carries.
    pub attachments: Vec<Attachment>,
    /// Suggestions taken, turned into mentions when the message is sent.
    pub completions: Vec<Completion>,
    /// The slash command picked for the text, run on Enter.
    pub command: Option<AppCommand>,
    /// The channel the box is writing to now.
    channel: Option<Snowflake>,
    /// What was left unsent in other channels.
    drafts: Drafts<Draft>,
}

/// An unsent message, put back when its channel opens again.
struct Draft {
    text: String,
    reply: Option<Replying>,
    attachments: Vec<Attachment>,
    completions: Vec<Completion>,
    command: Option<AppCommand>,
}

impl Composer {
    pub fn new(cx: &mut Context<Emerald>) -> Self {
        Self {
            input: cx.new(|cx| TextInput::new(t!("composer.no_channel"), cx).submit_empty().multiline(MAX_ROWS)),
            draft: SharedString::default(),
            error: None,
            reply: None,
            attachments: Vec::new(),
            completions: Vec::new(),
            command: None,
            channel: None,
            drafts: Drafts::default(),
        }
    }

    /// `channel` was opened: what was being written for the last one is
    /// kept, and this one's draft comes back.
    pub fn open(&mut self, channel: Snowflake, placeholder: String, cx: &mut Context<Emerald>) {
        self.error = None;
        self.input
            .update(cx, |input, cx| input.set_placeholder(placeholder, cx));
        if self.channel == Some(channel) {
            return;
        }
        let text = self.input.read(cx).text().to_owned();
        let left = Draft {
            text,
            reply: self.reply.take(),
            attachments: std::mem::take(&mut self.attachments),
            completions: std::mem::take(&mut self.completions),
            command: self.command.take(),
        };
        if let Some(previous) = self.channel
            && (!left.text.trim().is_empty() || left.reply.is_some() || !left.attachments.is_empty())
        {
            self.drafts.keep(previous, left);
        }
        self.channel = Some(channel);
        let draft = self.drafts.take(channel);
        let text = draft.as_ref().map(|d| d.text.clone()).unwrap_or_default();
        // Known before the input reports the change, so putting a draft back
        // does not tell the channel we are typing.
        self.draft = text.clone().into();
        self.input.update(cx, |input, cx| input.set_text(text, cx));
        if let Some(draft) = draft {
            self.reply = draft.reply;
            self.attachments = draft.attachments;
            self.completions = draft.completions;
            self.command = draft.command;
        }
    }

    /// Puts `content` and `files` back unless the user already moved on.
    fn give_back(&mut self, content: String, files: Vec<Upload>, cx: &mut Context<Emerald>) {
        self.input.update(cx, |input, cx| {
            if input.text().is_empty() {
                input.set_text(content, cx);
            }
        });
        if self.attachments.is_empty() {
            self.attachments = files.into_iter().map(Attachment::new).collect();
        }
    }
}

impl Emerald {
    pub(super) fn send(&mut self, content: String, cx: &mut Context<Self>) {
        let Some(channel) = self.selected_channel else { return };
        if !self.can_send(channel) {
            return;
        }
        let typed = content.trim_end().to_owned();
        if typed.trim().is_empty() && self.composer.attachments.is_empty() {
            return;
        }
        // Picked suggestions become mentions; Discord counts those.
        let content = autocomplete::expand(&typed, &self.composer.completions);
        if self.run_typed_command(&content, channel, cx) {
            self.composer.completions.clear();
            return;
        }
        if content.chars().count() > MAX_MESSAGE_LEN {
            self.composer.error =
                Some(t!("composer.too_long", max = MAX_MESSAGE_LEN).into());
            self.composer.input.update(cx, |input, cx| input.set_text(typed, cx));
            cx.notify();
            return;
        }
        self.composer.completions.clear();
        self.composer.error = None;
        self.typing.sent();
        let nonce = rest::nonce();
        let reply = self.composer.reply.take().filter(|r| r.channel == channel);
        let files: Vec<Upload> = self.composer.attachments.drain(..).map(|a| a.upload).collect();
        if let Some(me) = self.people.me() {
            let preview = reply.as_ref().map(|r| r.preview.clone());
            let view = shown(&self.messages, self.selected_channel, channel);
            let mut pending = Msg::pending(me, content.clone(), nonce.clone(), preview);
            pending.files = files.iter().map(|f| f.name.clone().into()).collect();
            self.history.add_pending(channel, pending, view);
        }
        self.net.post(Command::Send {
            channel,
            content,
            nonce,
            reply: reply.map(|r| ReplyTo {
                message: r.message,
                mention: r.mention,
            }),
            files,
        });
        cx.notify();
    }

    /// The words in the message box for `channel`, as the web client puts them.
    pub(super) fn composer_placeholder(&self, channel: Snowflake) -> String {
        let space = self.spaces.current();
        // Forum posts have no sidebar row; their name is the thread's.
        let name = space.row(channel).map(|row| row.name.clone()).unwrap_or_else(|| {
            let thread = space.channels().iter().find(|c| c.id == channel);
            thread.and_then(|c| c.name.clone()).unwrap_or_default().into()
        });
        if !self.can_send(channel) {
            t!("composer.no_permission").to_owned()
        } else if space.is_dms() {
            t!("composer.dm", name = name)
        } else {
            t!("composer.channel", name = name)
        }
    }

    /// Permissions changed: the open channel may now be writable or not.
    pub(super) fn refresh_composer(&mut self, cx: &mut Context<Self>) {
        if let Some(channel) = self.selected_channel.filter(|_| !self.friends_open) {
            let placeholder = self.composer_placeholder(channel);
            self.composer.input.update(cx, |input, cx| input.set_placeholder(placeholder, cx));
        }
        cx.notify();
    }

    pub(super) fn on_send_failed(
        &mut self,
        channel: Snowflake,
        nonce: &str,
        content: String,
        files: Vec<Upload>,
        error: String,
        cx: &mut Context<Self>,
    ) {
        let view = shown(&self.messages, self.selected_channel, channel);
        self.history.drop_pending(channel, nonce, view);
        if self.selected_channel == Some(channel) {
            self.composer.error = Some(error.into());
            self.composer.give_back(content, files, cx);
        }
    }

    /// Sends the typing signal while the user edits a draft, throttled per
    /// channel like the web client.
    pub(super) fn on_draft_changed(&mut self, cx: &mut Context<Self>) {
        self.refresh_suggestions(false, cx);
        let text = self.composer.input.read(cx).text();
        if text == self.composer.draft.as_ref() {
            return;
        }
        self.composer.draft = text.to_owned().into();
        let Some(channel) = self.selected_channel else { return };
        if self.composer.draft.trim().is_empty() || self.history.is_forbidden(channel) {
            return;
        }
        if self.typing.should_signal(channel, std::time::Instant::now()) {
            self.net.post(Command::Typing { channel });
        }
    }
}
