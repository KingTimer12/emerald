//! The reaction picker: a search box over "Frequently Used", the custom
//! emoji we may use, and the Unicode categories, with the hovered emoji
//! named at the bottom, as in Discord.

use std::ops::Range;
use std::rc::Rc;

use gpui::{
    AnyElement, Context, Focusable, FontWeight, Pixels, Point, SharedString, Window, anchored, deferred, div,
    prelude::*, px,
    rgb, uniform_list,
};

use super::super::super::{emoji, theme};
use super::super::Emerald;
use super::reactions::emoji_label;
use crate::discord::ReactionSource;
use crate::discord::model::{Emoji, Snowflake};

const COLUMNS: usize = 9;
/// Every row, headers included, has this height: `uniform_list` needs one.
const CELL: f32 = 40.;
const EMOJI: f32 = 28.;
const HEIGHT: f32 = 420.;

/// The picker, open for one message.
pub(in super::super) struct ReactionPicker {
    pub channel: Snowflake,
    pub message: Snowflake,
    pub position: Point<Pixels>,
    /// Named in the footer.
    pub hovered: Option<Emoji>,
}

enum Row {
    Header(SharedString),
    Emojis(Vec<Emoji>),
    Empty,
}

impl Emerald {
    pub(super) fn open_reaction_picker(
        &mut self,
        channel: Snowflake,
        message: Snowflake,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.message_menu = None;
        self.reaction_picker = Some(ReactionPicker {
            channel,
            message,
            position,
            hovered: None,
        });
        self.emoji_query.update(cx, |input, cx| input.set_text("", cx));
        self.emoji_scroll = gpui::UniformListScrollHandle::new();
        window.focus(&self.emoji_query.focus_handle(cx));
        cx.notify();
    }

    pub(in super::super) fn close_reaction_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.reaction_picker.take().is_none() {
            return false;
        }
        window.focus(&self.composer.input.focus_handle(cx));
        cx.notify();
        true
    }

    /// Reacts with `emoji`; shift keeps the picker open for more, as in Discord.
    fn pick_emoji(&mut self, emoji: Emoji, keep_open: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(picker) = &self.reaction_picker else { return };
        let (channel, message) = (picker.channel, picker.message);
        self.toggle_reaction(channel, message, emoji, ReactionSource::Picker, cx);
        if !keep_open {
            self.close_reaction_picker(window, cx);
        }
    }

    /// Enter in the search box picks the first match.
    pub(in super::super) fn pick_first_emoji(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let first = self.picker_rows(cx).into_iter().find_map(|row| match row {
            Row::Emojis(emojis) => emojis.into_iter().next(),
            _ => None,
        });
        if let Some(emoji) = first {
            self.pick_emoji(emoji, false, window, cx);
        }
    }

    /// Custom emoji we may use here: without Nitro only this guild's, and
    /// none animated; with it, every guild's.
    fn custom_emoji(&self) -> Vec<(SharedString, Vec<Emoji>)> {
        let nitro = self.people.me().and_then(|me| me.premium_type).unwrap_or(0) > 0;
        let current = self.spaces.current().id;
        self.spaces
            .iter()
            .filter(|space| nitro || space.id == current)
            .filter_map(|space| {
                let emojis: Vec<Emoji> = space
                    .info()?
                    .emojis()
                    .iter()
                    .filter(|e| e.available && (nitro || !e.animated))
                    .map(|e| e.emoji())
                    .collect();
                (!emojis.is_empty()).then(|| (space.name.clone(), emojis))
            })
            .collect()
    }

    fn picker_rows(&self, cx: &Context<Self>) -> Vec<Row> {
        let query = self.emoji_query.read(cx).text().trim().to_lowercase();
        let mut rows = Vec::new();
        let section = |rows: &mut Vec<Row>, header: Option<SharedString>, emojis: Vec<Emoji>| {
            if emojis.is_empty() {
                return;
            }
            rows.extend(header.map(Row::Header));
            rows.extend(emojis.chunks(COLUMNS).map(|chunk| Row::Emojis(chunk.to_vec())));
        };
        let custom = self.custom_emoji();
        if query.is_empty() {
            section(&mut rows, Some(t!("reaction.frequent").into()), self.recent_emojis().collect());
            for (guild, emojis) in custom {
                section(&mut rows, Some(guild), emojis);
            }
            for (category, emojis) in emoji::by_category() {
                let emojis = emojis.iter().map(|e| Emoji::unicode(e.as_str())).collect();
                section(&mut rows, Some(category.label().into()), emojis);
            }
        } else {
            let mut found: Vec<Emoji> = custom
                .into_iter()
                .flat_map(|(_, emojis)| emojis)
                .filter(|e| e.name.as_deref().is_some_and(|n| n.to_lowercase().contains(&query)))
                .collect();
            found.extend(
                emoji::by_category()
                    .iter()
                    .flat_map(|(_, emojis)| emojis)
                    .filter(|e| emoji::matches(e, &query))
                    .map(|e| Emoji::unicode(e.as_str())),
            );
            section(&mut rows, None, found);
            if rows.is_empty() {
                rows.push(Row::Empty);
            }
        }
        rows
    }

    pub(in super::super) fn render_reaction_picker(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let picker = self.reaction_picker.as_ref()?;
        let rows = Rc::new(self.picker_rows(cx));
        let count = rows.len();
        let grid = uniform_list(
            "emoji-grid",
            count,
            cx.processor(move |this, range: Range<usize>, _, cx| {
                range.map(|ix| this.render_picker_row(&rows[ix], ix, cx)).collect::<Vec<_>>()
            }),
        )
        .track_scroll(self.emoji_scroll.clone())
        .size_full();
        let footer = picker.hovered.as_ref().map(|emoji| {
            div()
                .h(px(48.))
                .flex_none()
                .px_3()
                .flex()
                .items_center()
                .gap_2()
                .border_t_1()
                .border_color(theme::border())
                .bg(rgb(theme::BASE_LOWEST))
                .child(self.emoji_image(emoji, EMOJI))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .truncate()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(emoji_label(emoji)),
                )
        });
        let panel = div()
            .id("reaction-picker")
            .w(px(COLUMNS as f32 * CELL + 24.))
            .h(px(HEIGHT))
            .flex()
            .flex_col()
            .rounded_lg()
            .overflow_hidden()
            .border_1()
            .border_color(theme::border())
            .bg(rgb(theme::SURFACE_HIGH))
            .shadow_lg()
            .occlude()
            .on_mouse_down_out(cx.listener(|this, _, window, cx| {
                this.close_reaction_picker(window, cx);
            }))
            .child(div().flex_none().p_2().child(self.emoji_query.clone()))
            .child(div().flex_1().min_h(px(0.)).px_2().child(grid))
            .children(footer);
        Some(
            deferred(anchored().position(picker.position).snap_to_window_with_margin(px(8.)).child(panel))
                .with_priority(2)
                .into_any_element(),
        )
    }

    fn render_picker_row(&self, row: &Row, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let line = div().h(px(CELL)).w_full().flex().items_center();
        match row {
            Row::Header(label) => line
                .pt_2()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::muted())
                .child(label.clone())
                .into_any_element(),
            Row::Empty => line
                .justify_center()
                .text_color(theme::muted())
                .child(t!("reaction.no_results"))
                .into_any_element(),
            Row::Emojis(emojis) => line
                .children(emojis.iter().enumerate().map(|(n, emoji)| {
                    let (picked, hovered) = (emoji.clone(), emoji.clone());
                    div()
                        .id(("emoji", ix * COLUMNS + n))
                        .size(px(CELL))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_md()
                        .cursor_pointer()
                        .hover(|s| s.bg(theme::hover()))
                        .child(self.emoji_image(emoji, EMOJI))
                        .on_hover(cx.listener(move |this, inside: &bool, _, cx| {
                            if let Some(picker) = &mut this.reaction_picker
                                && *inside
                            {
                                picker.hovered = Some(hovered.clone());
                                cx.notify();
                            }
                        }))
                        .on_click(cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                            this.pick_emoji(picked.clone(), event.modifiers().shift, window, cx)
                        }))
                }))
                .into_any_element(),
        }
    }
}
