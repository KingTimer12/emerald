//! The message box under the chat: sending, the typing signal, and errors.

use gpui::{AppContext, Context, Entity, SharedString};

use super::chat::Replying;
use super::state::Msg;
use super::{Emerald, shown};
use super::super::text_input::TextInput;
use crate::discord::model::Snowflake;
use crate::discord::{Command, ReplyTo, rest};

const MAX_MESSAGE_LEN: usize = 2000;

pub(super) struct Composer {
    pub input: Entity<TextInput>,
    /// Draft text when the input last changed, to tell edits from cursor moves.
    draft: SharedString,
    /// Why the last message in the open channel was not sent.
    pub error: Option<SharedString>,
    /// The message the next send answers.
    pub reply: Option<Replying>,
}

impl Composer {
    pub fn new(cx: &mut Context<Emerald>) -> Self {
        Self {
            input: cx.new(|cx| TextInput::new(t!("composer.no_channel"), cx)),
            draft: SharedString::default(),
            error: None,
            reply: None,
        }
    }

    /// A channel was opened.
    pub fn open(&mut self, placeholder: String, cx: &mut Context<Emerald>) {
        self.error = None;
        self.reply = None;
        self.input
            .update(cx, |input, cx| input.set_placeholder(placeholder, cx));
    }

    /// Puts `content` back in the box unless the user already typed more.
    fn give_back(&self, content: String, cx: &mut Context<Emerald>) {
        self.input.update(cx, |input, cx| {
            if input.text().is_empty() {
                input.set_text(content, cx);
            }
        });
    }
}

impl Emerald {
    pub(super) fn send(&mut self, content: String, cx: &mut Context<Self>) {
        let Some(channel) = self.selected_channel else { return };
        if !self.can_send(channel) {
            return;
        }
        let content = content.trim_end().to_owned();
        if content.trim().is_empty() {
            return;
        }
        if content.chars().count() > MAX_MESSAGE_LEN {
            self.composer.error =
                Some(t!("composer.too_long", max = MAX_MESSAGE_LEN).into());
            self.composer.input.update(cx, |input, cx| input.set_text(content, cx));
            cx.notify();
            return;
        }
        self.composer.error = None;
        self.typing.sent();
        let nonce = rest::nonce();
        let reply = self.composer.reply.take().filter(|r| r.channel == channel);
        if let Some(me) = self.people.me() {
            let preview = reply.as_ref().map(|r| r.preview.clone());
            let view = shown(&self.messages, self.selected_channel, channel);
            self.history
                .add_pending(channel, Msg::pending(me, content.clone(), nonce.clone(), preview), view);
        }
        self.net.post(Command::Send {
            channel,
            content,
            nonce,
            reply: reply.map(|r| ReplyTo {
                message: r.message,
                mention: r.mention,
            }),
        });
        cx.notify();
    }

    /// The words in the message box for `channel`, as the web client puts them.
    pub(super) fn composer_placeholder(&self, channel: Snowflake) -> String {
        let space = self.spaces.current();
        let name = space.row(channel).map_or_else(SharedString::default, |row| row.name.clone());
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
        error: String,
        cx: &mut Context<Self>,
    ) {
        let view = shown(&self.messages, self.selected_channel, channel);
        self.history.drop_pending(channel, nonce, view);
        if self.selected_channel == Some(channel) {
            self.composer.error = Some(error.into());
            self.composer.give_back(content, cx);
        }
    }

    /// Sends the typing signal while the user edits a draft, throttled per
    /// channel like the web client.
    pub(super) fn on_draft_changed(&mut self, cx: &mut Context<Self>) {
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
