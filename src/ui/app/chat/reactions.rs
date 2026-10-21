//! Reactions: the chips under a message, adding and removing ours, and
//! keeping counts in step with the gateway. Ours show at once and are
//! undone if Discord refuses them.

use gpui::{Context, Div, FontWeight, SharedString, div, img, prelude::*, px, rgb};

use super::super::super::icons::{self, icon};
use super::super::super::{emoji, theme};
use super::super::state::{Msg, Reacted};
use super::super::{Emerald, shown, widgets};
use crate::discord::model::{Emoji, Snowflake, custom_url};
use crate::discord::{Command, ReactionEvent, ReactionSource, permissions};

/// Emoji kept for "Frequently Used", newest first.
const RECENT_EMOJI: usize = 27;
/// Frequently used emoji offered on the hover toolbar.
pub(super) const QUICK_REACTIONS: usize = 3;
const CHIP_EMOJI: f32 = 16.;

impl Emerald {
    /// A reaction event from the gateway.
    pub(in super::super) fn on_reaction(&mut self, event: ReactionEvent) {
        let me = self.people.my_id();
        let (channel, message) = match &event {
            ReactionEvent::Add(u) | ReactionEvent::Remove(u) => (u.channel_id, u.message_id),
            ReactionEvent::AddMany(batch) => (batch.channel_id, batch.message_id),
            ReactionEvent::Clear(clear) => (clear.channel_id, clear.message_id),
        };
        let view = shown(&self.messages, self.selected_channel, channel);
        self.history.update(channel, message, view, |msg| match event {
            ReactionEvent::Add(u) => msg.react(&u.emoji, u.burst, u.user_id == me, true),
            ReactionEvent::Remove(u) => msg.react(&u.emoji, u.burst, u.user_id == me, false),
            ReactionEvent::AddMany(batch) => batch.reactions.iter().fold(false, |changed, reaction| {
                reaction.users.iter().fold(changed, |changed, user| {
                    msg.react(&reaction.emoji, reaction.burst, *user == me, true) | changed
                })
            }),
            ReactionEvent::Clear(clear) => msg.clear_reactions(clear.emoji.as_ref()),
        });
    }

    /// Discord refused our reaction: put the count back and say why.
    pub(in super::super) fn reaction_failed(
        &mut self,
        channel: Snowflake,
        message: Snowflake,
        emoji: &Emoji,
        add: bool,
        error: String,
    ) {
        let view = shown(&self.messages, self.selected_channel, channel);
        self.history.update(channel, message, view, |msg| msg.react(emoji, false, true, !add));
        self.composer.error = Some(error.into());
    }

    /// Adds our reaction with `emoji`, or takes it back if it is there.
    pub(super) fn toggle_reaction(
        &mut self,
        channel: Snowflake,
        message: Snowflake,
        emoji: Emoji,
        source: ReactionSource,
        cx: &mut Context<Self>,
    ) {
        let Some((_, msg)) = self.history.find(channel, message) else { return };
        let existing = msg.reactions.iter().find(|r| r.emoji.same(&emoji));
        let add = !existing.is_some_and(|r| r.me);
        if add && !self.may_react(channel, msg, existing.is_none()) {
            return;
        }
        let view = shown(&self.messages, self.selected_channel, channel);
        self.history.update(channel, message, view, |msg| msg.react(&emoji, false, true, add));
        if add {
            self.remember_emoji(&emoji);
        }
        self.net.post(Command::React {
            channel,
            message,
            emoji,
            add,
            source,
        });
        cx.notify();
    }

    /// Whether we may react to `msg`: a reaction already there needs only
    /// the history, a new emoji also Add Reactions. Everything goes in DMs.
    pub(super) fn may_react(&self, channel: Snowflake, msg: &Msg, new_emoji: bool) -> bool {
        if msg.pending || msg.id == Snowflake(0) {
            return false;
        }
        let space = self.spaces.current();
        if space.is_dms() {
            return true;
        }
        let needed = permissions::READ_MESSAGE_HISTORY | if new_emoji { permissions::ADD_REACTIONS } else { 0 };
        space.permissions(channel) & needed == needed
    }

    /// Moves `emoji` to the front of "Frequently Used".
    fn remember_emoji(&mut self, emoji: &Emoji) {
        let key = emoji.route_key();
        let recent = &mut self.settings.recent_emojis;
        recent.retain(|k| *k != key);
        recent.insert(0, key);
        recent.truncate(RECENT_EMOJI);
        if let Err(e) = self.settings.save() {
            tracing::error!("could not save settings: {e}");
        }
    }

    /// "Frequently Used", as emoji.
    pub(super) fn recent_emojis(&self) -> impl Iterator<Item = Emoji> + '_ {
        self.settings.recent_emojis.iter().map(|key| emoji_from_key(key))
    }

    /// The chips under a message, then a button to add another.
    pub(super) fn render_reactions(&self, msg: &Msg, ix: usize, cx: &mut Context<Self>) -> Option<Div> {
        if msg.reactions.is_empty() {
            return None;
        }
        let channel = self.selected_channel?;
        let id = msg.id;
        let mut chips = Vec::new();
        for (n, reaction) in msg.reactions.iter().enumerate() {
            // Super reactions get a chip of their own, after the normal one.
            if reaction.normal > 0 {
                let enabled = reaction.me || self.may_react(channel, msg, false);
                let emoji = reaction.emoji.clone();
                chips.push(
                    self.reaction_chip(reaction, false, (ix, n * 2))
                        .when(enabled, |chip| {
                            chip.cursor_pointer().on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_reaction(channel, id, emoji.clone(), ReactionSource::Message, cx)
                            }))
                        })
                        .into_any_element(),
                );
            }
            if reaction.burst > 0 {
                chips.push(self.reaction_chip(reaction, true, (ix, n * 2 + 1)).into_any_element());
            }
        }
        let add = self.may_react(channel, msg, true).then(|| {
            div()
                .id(("reaction-add", ix))
                .h(px(28.))
                .px_2()
                .flex()
                .items_center()
                .rounded_lg()
                .cursor_pointer()
                .border_1()
                .border_color(gpui::transparent_black())
                .bg(rgb(theme::BASE_LOW))
                .hover(|s| s.border_color(theme::border()))
                .tooltip(widgets::tooltip(t!("reaction.add")))
                .child(icon(icons::REACTION, 18., theme::INTERACTIVE))
                .on_click(cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                    this.open_reaction_picker(channel, id, event.position(), window, cx)
                }))
        });
        Some(div().flex().flex_wrap().gap_1().pt_1().children(chips).children(add))
    }

    fn reaction_chip(&self, reaction: &Reacted, burst: bool, id: (usize, usize)) -> gpui::Stateful<Div> {
        let (count, mine) = if burst {
            (reaction.burst, reaction.me_burst)
        } else {
            (reaction.normal, reaction.me)
        };
        let name = emoji_label(&reaction.emoji);
        div()
            .id(SharedString::from(format!("reaction-{}-{}", id.0, id.1)))
            .h(px(28.))
            .px_2()
            .flex()
            .items_center()
            .gap(px(6.))
            .rounded_lg()
            .border_1()
            .map(|chip| {
                if mine {
                    chip.bg(theme::reaction_mine()).border_color(rgb(theme::BRAND))
                } else {
                    chip.bg(rgb(theme::BASE_LOW))
                        .border_color(gpui::transparent_black())
                        .hover(|s| s.border_color(theme::border()))
                }
            })
            // Super reactions: Discord's gradient outline, approximated.
            .when(burst && !mine, |chip| chip.border_color(rgb(theme::IDLE)))
            .tooltip(widgets::tooltip(name))
            .child(self.emoji_image(&reaction.emoji, CHIP_EMOJI))
            .child(
                div()
                    .text_size(px(14.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(if mine { rgb(theme::MENTION_TEXT).into() } else { theme::muted() })
                    .child(count.to_string()),
            )
    }

    /// An emoji at `size`: Unicode ones as text, custom ones as their image
    /// (a blank square while it downloads).
    pub(super) fn emoji_image(&self, emoji: &Emoji, size: f32) -> gpui::AnyElement {
        match emoji.id {
            Some(id) => match self.avatar_image(&custom_url(id)) {
                Some(image) => img(image).size(px(size)).flex_none().into_any_element(),
                None => div().size(px(size)).flex_none().into_any_element(),
            },
            None => div()
                .flex_none()
                .text_size(px(size))
                .line_height(px(size * 1.2))
                .child(emoji.name.clone().unwrap_or_default())
                .into_any_element(),
        }
    }
}

/// `:name:`, as Discord labels an emoji.
pub(super) fn emoji_label(emoji: &Emoji) -> String {
    let name = match (&emoji.id, emoji.name.as_deref()) {
        (Some(_), Some(name)) => name,
        (None, Some(text)) => emoji::name(text).unwrap_or(text),
        (_, None) => "?",
    };
    format!(":{name}:")
}

/// The emoji a "Frequently Used" entry names: `name:id` for custom ones.
fn emoji_from_key(key: &str) -> Emoji {
    match key.rsplit_once(':').and_then(|(name, id)| Some((name, id.parse().ok()?))) {
        Some((name, id)) => Emoji {
            id: Some(Snowflake(id)),
            name: Some(name.to_owned()),
            animated: false,
        },
        None => Emoji::unicode(key),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_keys_round_trip() {
        for emoji in [
            Emoji::unicode("👍"),
            Emoji {
                id: Some(Snowflake(42)),
                name: Some("pepe".into()),
                animated: false,
            },
        ] {
            assert!(emoji_from_key(&emoji.route_key()).same(&emoji));
        }
    }

    #[test]
    fn labels_use_shortcodes() {
        assert_eq!(emoji_label(&Emoji::unicode("👍")), ":thumbsup:");
        let custom = Emoji {
            id: Some(Snowflake(1)),
            name: Some("pepe".into()),
            animated: false,
        };
        assert_eq!(emoji_label(&custom), ":pepe:");
    }
}
