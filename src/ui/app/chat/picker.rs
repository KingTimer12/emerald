//! The emoji picker: a search box over "Frequently Used", the custom emoji
//! we may use, and the Unicode categories, with the hovered emoji named at
//! the bottom, as in Discord. It reacts to a message, or writes the emoji
//! into the message box from the button beside it.

use std::ops::Range;
use std::rc::Rc;

use gpui::{
    AnyElement, Context, Focusable, FontWeight, Pixels, Point, SharedString, Window, anchored, deferred, div,
    prelude::*, px,
    rgb, uniform_list,
};

use super::super::super::{emoji, icons, theme};
use super::super::{Emerald, widgets};
use super::expressions::{GifResults, StickerList};
use super::reactions::emoji_label;
use crate::discord::ReactionSource;
use crate::discord::model::{Emoji, Snowflake};

const COLUMNS: usize = 9;
/// Every row, headers included, has this height: `uniform_list` needs one.
const CELL: f32 = 40.;
const EMOJI: f32 = 28.;
pub(super) const HEIGHT: f32 = 420.;
/// The panel's width, shared by every tab.
pub(super) const WIDTH: f32 = COLUMNS as f32 * CELL + 24.;

/// The picker, open for one message, or for the message box when
/// `message` is `None`.
pub(in super::super) struct ReactionPicker {
    pub channel: Snowflake,
    pub message: Option<Snowflake>,
    pub position: Point<Pixels>,
    /// Named in the footer.
    pub hovered: Option<Emoji>,
    /// GIFs, stickers or emoji; only the message box offers all three.
    pub tab: PickerTab,
    pub gifs: GifResults,
    pub stickers: StickerList,
}

/// The picker's tabs, in the web client's order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in super::super) enum PickerTab {
    Gifs,
    Stickers,
    #[default]
    Emoji,
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
        self.open_picker(channel, Some(message), position, window, cx);
    }

    /// The emoji button in the message box.
    pub(in super::super) fn open_composer_picker(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(channel) = self.selected_channel else { return };
        // Opens over the chat, up and to the left of the button and clear
        // of the message box.
        let position = position - gpui::point(px(WIDTH), px(HEIGHT + 24.));
        self.open_picker(channel, None, position, window, cx);
    }

    fn open_picker(
        &mut self,
        channel: Snowflake,
        message: Option<Snowflake>,
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
            tab: PickerTab::Emoji,
            gifs: GifResults::default(),
            stickers: StickerList::default(),
        });
        self.emoji_query.update(cx, |input, cx| {
            input.set_text("", cx);
            input.set_placeholder(t!("reaction.search"), cx);
        });
        self.emoji_scroll = gpui::UniformListScrollHandle::new();
        window.focus(&self.emoji_query.focus_handle(cx));
        cx.notify();
    }

    pub(in super::super) fn close_reaction_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.reaction_picker.take().is_none() {
            return false;
        }
        // Sticker stills evicted while browsing leave the GPU with the picker.
        for image in self.images.take_retired() {
            cx.drop_image(image, None);
        }
        window.focus(&self.composer.input.focus_handle(cx));
        cx.notify();
        true
    }

    /// Reacts with `emoji`, or writes it in the message box; shift keeps the
    /// picker open for more, as in Discord.
    fn pick_emoji(&mut self, emoji: Emoji, keep_open: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(picker) = &self.reaction_picker else { return };
        match (picker.channel, picker.message) {
            (channel, Some(message)) => self.toggle_reaction(channel, message, emoji, ReactionSource::Picker, cx),
            (channel, None) if self.selected_channel == Some(channel) => {
                self.remember_emoji(&emoji);
                self.insert_emoji(emoji, cx);
            }
            _ => {}
        }
        if !keep_open {
            self.close_reaction_picker(window, cx);
        }
    }

    /// Enter in the search box picks the first match.
    pub(in super::super) fn pick_first_emoji(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.reaction_picker.as_ref().map(|p| p.tab) {
            Some(PickerTab::Stickers) => return self.pick_first_sticker(window, cx),
            Some(PickerTab::Gifs) => return self.pick_first_gif(window, cx),
            _ => {}
        }
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
    pub(in super::super) fn custom_emoji(&self) -> Vec<(SharedString, Vec<Emoji>)> {
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
        let body = match picker.tab {
            PickerTab::Emoji => grid.into_any_element(),
            PickerTab::Stickers => self.render_sticker_tab(cx),
            PickerTab::Gifs => self.render_gif_tab(cx),
        };
        let tabs = picker.message.is_none().then(|| {
            let tab = |id: &'static str, label: &'static str, which: PickerTab| {
                let on = picker.tab == which;
                div()
                    .id(id)
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .font_weight(FontWeight::MEDIUM)
                    .text_size(px(14.))
                    .text_color(rgb(if on { theme::TEXT_STRONG } else { theme::TEXT_MUTED }))
                    .when(on, |tab| tab.bg(theme::selected()))
                    .hover(|tab| tab.bg(theme::hover()))
                    .child(label)
                    .on_click(cx.listener(move |this, _, window, cx| this.switch_picker_tab(which, window, cx)))
            };
            div()
                .flex_none()
                .flex()
                .gap_1()
                .px_2()
                .pt_2()
                .child(tab("picker-gifs", t!("picker.gifs"), PickerTab::Gifs))
                .child(tab("picker-stickers", t!("picker.stickers"), PickerTab::Stickers))
                .child(tab("picker-emoji", t!("picker.emoji"), PickerTab::Emoji))
        });
        let footer = picker.hovered.as_ref().filter(|_| picker.tab == PickerTab::Emoji).map(|emoji| {
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
            .w(px(WIDTH))
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
            .children(tabs)
            .child(div().flex_none().p_2().child(self.emoji_query.clone()))
            .child(div().flex_1().min_h(px(0.)).px_2().child(body))
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

impl Emerald {
    /// The emoji button at the end of the message box.
    pub(super) fn render_emoji_button(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let channel = self.selected_channel?;
        if !self.can_send(channel) {
            return None;
        }
        let open = self.reaction_picker.as_ref().is_some_and(|p| p.message.is_none());
        Some(
            div()
                .id("composer-emoji")
                .flex_none()
                .mt(px(7.))
                .size(px(32.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_md()
                .cursor_pointer()
                .hover(|s| s.bg(theme::hover()))
                .tooltip(widgets::tooltip(t!("composer.emoji")))
                .child(icons::icon(icons::REACTION, 22., if open { theme::TEXT_STRONG } else { theme::INTERACTIVE }))
                .on_click(cx.listener(|this, event: &gpui::ClickEvent, window, cx| {
                    this.open_composer_picker(event.position(), window, cx)
                }))
                .into_any_element(),
        )
    }
}
