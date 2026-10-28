//! Message text: Discord markdown turned into blocks of styled, clickable
//! text. Custom emoji are painted over room kept for them in the text,
//! since gpui text cannot hold images.

mod blocks;
mod style;
mod text;

use style::*;
use text::*;

use std::ops::Range;
use std::time::Duration;

use gpui::{
    AnyElement, Bounds, ElementId, FontStyle, FontWeight, HighlightStyle, ImageSource, InteractiveText, SharedString,
    StrikethroughStyle, StyledText, TextLayout, UnderlineStyle, canvas, div, point, prelude::*, rgb, rgba, size,
};

use super::super::super::markdown::{self, Block, Mention, Rich, Style, TextKind};
use super::super::super::theme;
use super::super::Emerald;
use super::select::{self, Selectable};
use emerald_discord::model::{Snowflake, animated_custom_url, custom_url};

/// Emoji-only messages, as large as Discord draws them.
const JUMBO: f32 = 48.;
/// Custom emoji are fetched this many pixels square (`?size=48`).
const EMOJI_PIXELS: u32 = 48;
/// Spoilers opened by the reader are remembered up to this many, then the
/// oldest close again.
const MAX_REVEALED: usize = 500;

/// A text spoiler the reader opened: the message, which part of it (its
/// content or one of its embeds), the block and the spoiler's place in it.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct SpoilerKey {
    message: Snowflake,
    part: u32,
    block: u32,
    spoiler: u32,
}

/// How a text is drawn and what it lets the reader do.
#[derive(Default)]
struct Options<'a> {
    mentions: &'a [(Snowflake, SharedString)],
    /// Links, mentions and spoilers respond to clicks.
    clickable: bool,
    /// Where opened spoilers are remembered; without it they stay closed.
    scope: Option<(Snowflake, u32)>,
    /// A message's own text: selectable, and large when only emoji.
    own: Option<Snowflake>,
    /// Muted words after the text, like "(edited)".
    note: Option<&'a str>,
}

/// What a click on a range of text does.
#[derive(Clone)]
enum Click {
    Reveal(SpoilerKey),
    Open(String),
    /// A `discord.com/channels/…` link, opened here.
    Link(Option<Snowflake>, Snowflake, Option<Snowflake>),
    Channel(Snowflake),
    User(Snowflake),
}

impl Emerald {
    /// One line of inline markdown, for a channel topic or a reply preview.
    /// Nothing in it responds to clicks.
    pub(super) fn rich_line(
        &self,
        id: impl Into<ElementId>,
        content: &str,
        mentions: &[(Snowflake, SharedString)],
    ) -> AnyElement {
        let options = Options {
            mentions,
            ..Options::default()
        };
        let rich = markdown::parse(content, &|mention| self.mention_name(mention, mentions));
        self.render_piece(id.into(), rich, 0, &options)
    }

    /// Message text in full, for previews: blocks drawn, nothing clickable.
    pub(in super::super) fn rich_text(
        &self,
        id: impl Into<ElementId>,
        content: &str,
        mentions: &[(Snowflake, SharedString)],
    ) -> AnyElement {
        self.render_blocks(
            id.into(),
            content,
            Options {
                mentions,
                ..Options::default()
            },
        )
    }

    /// A text inside an embed or a forward of `message`; `part` tells
    /// them apart.
    pub(super) fn rich_embed(
        &self,
        id: impl Into<ElementId>,
        content: &str,
        mentions: &[(Snowflake, SharedString)],
        message: Snowflake,
        part: u32,
    ) -> AnyElement {
        self.render_blocks(
            id.into(),
            content,
            Options {
                mentions,
                clickable: true,
                scope: Some((message, part)),
                ..Options::default()
            },
        )
    }

    /// A message's own text: clickable, selectable by dragging over it,
    /// large when only emoji, and followed by a muted `note`, as "(edited)"
    /// trails the text it marks.
    pub(super) fn rich_text_noted(
        &self,
        id: impl Into<ElementId>,
        content: &str,
        mentions: &[(Snowflake, SharedString)],
        note: Option<&str>,
        message: Snowflake,
    ) -> AnyElement {
        self.render_blocks(
            id.into(),
            content,
            Options {
                mentions,
                clickable: true,
                scope: Some((message, 0)),
                own: Some(message),
                note,
            },
        )
    }

    /// The name a mention shows: the member's nickname in this guild, the
    /// name the message carried, or the channel's or role's name.
    fn mention_name(&self, mention: Mention, mentions: &[(Snowflake, SharedString)]) -> String {
        let guild = self.spaces.current().info();
        match mention {
            Mention::User(id) => guild
                .and_then(|g| g.member_style(id).0.map(str::to_owned))
                .or_else(|| mentions.iter().find(|(m, _)| *m == id).map(|(_, n)| n.to_string()))
                .or_else(|| self.people.display_name(id).map(str::to_owned))
                .unwrap_or_else(|| t!("chat.unknown_user").to_owned()),
            Mention::Channel(id) => self
                .spaces
                .channel(id)
                .and_then(|c| c.name.clone())
                .unwrap_or_else(|| t!("chat.unknown_channel").to_owned()),
            Mention::Role(id) => guild
                .and_then(|g| g.role(id))
                .map_or_else(|| t!("chat.unknown_role").to_owned(), |r| r.name.clone()),
        }
    }
}
