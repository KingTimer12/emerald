//! The picker's other tabs, offered from the message box: stickers, sent
//! on their own when clicked, and GIFs from Discord's Tenor proxy, sent as
//! their Tenor link, both as the web client does.

use std::ops::Range;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AnyElement, Context, FontWeight, ListAlignment, ListState, SharedString, Window, div, img, list, prelude::*,
    uniform_list,
};

use super::super::super::theme;
use super::super::Emerald;
use super::super::state::Sticker;
use super::picker::{PickerTab, WIDTH};
use emerald_discord::model::{Gif, Snowflake, StickerItem};
use emerald_discord::{Command, permissions, rest};
use crate::zoom::zoomed;

/// The web client waits this long after a keystroke before searching GIFs.
const GIF_SEARCH_DELAY: Duration = Duration::from_millis(250);
const GIF_ROW: f32 = 120.;
const STICKER: f32 = 80.;
const STICKER_COLUMNS: usize = 4;

/// What the GIF tab shows: the answer for `query` (`None` for trending).
#[derive(Default)]
pub(in super::super) struct GifResults {
    query: Option<Option<String>>,
    items: Rc<Vec<Gif>>,
    loading: bool,
    error: Option<SharedString>,
}

/// The sticker tab's rows, and the list that scrolls them.
pub(in super::super) struct StickerList {
    state: ListState,
    /// What the rows were built for: the search and their count.
    built_for: std::cell::RefCell<(String, usize)>,
}

impl Default for StickerList {
    fn default() -> Self {
        Self {
            state: ListState::new(0, ListAlignment::Top, zoomed(200.)),
            built_for: Default::default(),
        }
    }
}

enum StickerRow {
    Header(SharedString),
    Stickers(Vec<StickerItem>),
}

impl Emerald {
    pub(super) fn switch_picker_tab(&mut self, tab: PickerTab, window: &mut Window, cx: &mut Context<Self>) {
        let Some(picker) = &mut self.reaction_picker else { return };
        if picker.tab == tab {
            return;
        }
        picker.tab = tab;
        picker.hovered = None;
        self.emoji_scroll = gpui::UniformListScrollHandle::new();
        let placeholder = match tab {
            PickerTab::Gifs => t!("picker.search_gifs"),
            PickerTab::Stickers => t!("picker.search_stickers"),
            PickerTab::Emoji => t!("reaction.search"),
        };
        self.emoji_query.update(cx, |input, cx| {
            input.set_text("", cx);
            input.set_placeholder(placeholder, cx);
        });
        window.focus(&gpui::Focusable::focus_handle(&self.emoji_query, cx));
        if tab == PickerTab::Stickers && self.sticker_packs.is_none() {
            self.sticker_packs = Some(Vec::new());
            self.net.post(Command::StickerPacks);
        }
        self.on_picker_query(cx);
        cx.notify();
    }

    /// The search box changed: the GIF tab searches after a pause, or shows
    /// the trending GIFs when it is empty.
    pub(in super::super) fn on_picker_query(&mut self, cx: &mut Context<Self>) {
        let Some(picker) = &mut self.reaction_picker else { return };
        if picker.tab != PickerTab::Gifs {
            return;
        }
        let text = self.emoji_query.read(cx).text().trim().to_owned();
        let query = (!text.is_empty()).then_some(text);
        if picker.gifs.query.as_ref() == Some(&query) {
            return;
        }
        picker.gifs.query = Some(query.clone());
        picker.gifs.loading = true;
        picker.gifs.error = None;
        let delay = if query.is_some() { GIF_SEARCH_DELAY } else { Duration::ZERO };
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;
            this.update(cx, |this, _| {
                let current = this.reaction_picker.as_ref().and_then(|p| p.gifs.query.clone());
                if current.as_ref() == Some(&query) {
                    this.net.post(Command::Gifs { query });
                }
            })
            .ok();
        })
        .detach();
    }

    /// GIFs arrived; kept if they still answer the search on screen.
    pub(in super::super) fn on_gifs(&mut self, query: Option<String>, gifs: Result<Vec<Gif>, String>, cx: &mut Context<Self>) {
        let Some(picker) = &mut self.reaction_picker else { return };
        if picker.gifs.query.as_ref() != Some(&query) {
            return;
        }
        picker.gifs.loading = false;
        match gifs {
            Ok(gifs) => picker.gifs.items = Rc::new(gifs),
            Err(error) => {
                picker.gifs.items = Rc::default();
                picker.gifs.error = Some(error.into());
            }
        }
        cx.notify();
    }

    /// Stickers we may send here, by guild: this guild's own, and with
    /// Nitro every guild's where outside stickers are allowed; then
    /// Discord's standard packs, which anyone may send.
    fn usable_stickers(&self, query: &str) -> Vec<(SharedString, Vec<StickerItem>)> {
        let nitro = self.people.me().and_then(|me| me.premium_type).unwrap_or(0) > 0;
        let space = self.spaces.current();
        let external = space.is_dms()
            || self
                .selected_channel
                .is_some_and(|c| space.permissions(c) & permissions::USE_EXTERNAL_STICKERS != 0);
        let matching = |stickers: &[StickerItem]| -> Vec<StickerItem> {
            stickers
                .iter()
                .filter(|s| s.available && Sticker::new(s).url.is_some())
                .filter(|s| {
                    query.is_empty() || s.name.to_lowercase().contains(query) || s.tags.to_lowercase().contains(query)
                })
                .cloned()
                .collect()
        };
        let guilds = self
            .spaces
            .iter()
            .filter(|other| other.id == space.id || (nitro && external))
            .filter_map(|other| Some((other.name.clone(), matching(other.info()?.stickers()))));
        let packs = self
            .sticker_packs
            .iter()
            .flatten()
            .map(|pack| (pack.name.clone().into(), matching(&pack.stickers)));
        guilds.chain(packs).filter(|(_, stickers)| !stickers.is_empty()).collect()
    }

    /// The picker's search, as stickers are matched against it.
    fn sticker_query(&self, cx: &Context<Self>) -> String {
        self.emoji_query.read(cx).text().trim().to_lowercase()
    }

    fn sticker_rows(&self, query: &str) -> Vec<StickerRow> {
        let mut rows = Vec::new();
        for (guild, stickers) in self.usable_stickers(query) {
            rows.push(StickerRow::Header(guild));
            rows.extend(stickers.chunks(STICKER_COLUMNS).map(|chunk| StickerRow::Stickers(chunk.to_vec())));
        }
        rows
    }

    /// Sends `sticker` to the open channel, alone, then closes the picker
    /// unless Shift is held.
    fn pick_sticker(&mut self, sticker: Snowflake, keep_open: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(channel) = self.reaction_picker.as_ref().map(|p| p.channel) else { return };
        if self.selected_channel != Some(channel) || !self.can_send(channel) {
            return;
        }
        self.typing.sent();
        self.net.post(Command::SendSticker {
            channel,
            sticker,
            nonce: rest::nonce(),
        });
        if !keep_open {
            self.close_reaction_picker(window, cx);
        }
    }

    /// Sends `gif` to the open channel as its Tenor link.
    fn pick_gif(&mut self, gif: Gif, window: &mut Window, cx: &mut Context<Self>) {
        let Some(picker) = &self.reaction_picker else { return };
        let channel = picker.channel;
        let query = picker.gifs.query.clone().flatten().unwrap_or_default();
        if self.selected_channel != Some(channel) || !self.can_send(channel) {
            return;
        }
        self.typing.sent();
        self.net.post(Command::SendGif {
            channel,
            gif,
            query,
            nonce: rest::nonce(),
        });
        self.close_reaction_picker(window, cx);
    }

    pub(super) fn pick_first_sticker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let query = self.sticker_query(cx);
        let first = self.usable_stickers(&query).into_iter().find_map(|(_, stickers)| stickers.into_iter().next());
        if let Some(sticker) = first {
            self.pick_sticker(sticker.id, false, window, cx);
        }
    }

    pub(super) fn pick_first_gif(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let first = self.reaction_picker.as_ref().and_then(|p| p.gifs.items.first().cloned());
        if let Some(gif) = first {
            self.pick_gif(gif, window, cx);
        }
    }

    pub(super) fn render_sticker_tab(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(picker) = &self.reaction_picker else { return div().into_any_element() };
        let query = self.sticker_query(cx);
        let rows = Rc::new(self.sticker_rows(&query));
        if rows.is_empty() {
            return empty(t!("picker.no_stickers"));
        }
        // The list keeps row heights by index; rebuilt rows start it over.
        let key = (query, rows.len());
        if *picker.stickers.built_for.borrow() != key {
            picker.stickers.state.reset(rows.len());
            *picker.stickers.built_for.borrow_mut() = key;
        }
        list(
            picker.stickers.state.clone(),
            cx.processor(move |this, ix: usize, _, cx| {
                let row = this.render_sticker_row(&rows[ix], cx);
                if this.animating.take() {
                    this.next_animation_frame(cx);
                }
                row
            }),
        )
        .size_full()
        .into_any_element()
    }

    fn render_sticker_row(&self, row: &StickerRow, cx: &mut Context<Self>) -> AnyElement {
        match row {
            StickerRow::Header(guild) => div()
                .pt_2()
                .pb_1()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::muted())
                .child(guild.clone())
                .into_any_element(),
            StickerRow::Stickers(stickers) => div()
                .flex()
                .gap_1()
                .children(stickers.iter().map(|sticker| {
                    let id = sticker.id;
                    div()
                        .id(("sticker", id.0))
                        .size(zoomed(STICKER + 4.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_md()
                        .cursor_pointer()
                        .hover(|s| s.bg(theme::hover()))
                        .tooltip(super::super::widgets::tooltip(sticker.name.clone()))
                        .child(self.render_picker_sticker(&Sticker::new(sticker)))
                        .on_click(cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                            this.pick_sticker(id, event.modifiers().shift, window, cx)
                        }))
                }))
                .into_any_element(),
        }
    }

    pub(super) fn render_gif_tab(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(picker) = &self.reaction_picker else { return div().into_any_element() };
        let results = &picker.gifs;
        if let Some(error) = &results.error {
            return empty(error.clone());
        }
        if results.items.is_empty() {
            return empty(if results.loading { t!("common.loading") } else { t!("picker.no_gifs") });
        }
        let gifs = results.items.clone();
        let rows = gifs.len().div_ceil(2);
        let cell = (WIDTH - 24. - 8.) / 2.;
        uniform_list(
            "gif-grid",
            rows,
            cx.processor(move |this, range: Range<usize>, _, cx| {
                range
                    .map(|row| {
                        div()
                            .h(zoomed(GIF_ROW))
                            .flex()
                            .gap_2()
                            .pb_2()
                            .children(gifs.iter().enumerate().skip(row * 2).take(2).map(|(ix, gif)| {
                                let picked = gif.clone();
                                let image = gif.image_url().and_then(|url| this.avatar_image(url));
                                div()
                                    .id(("gif", ix))
                                    .w(zoomed(cell))
                                    .h_full()
                                    .rounded_md()
                                    .overflow_hidden()
                                    .bg(theme::hover())
                                    .cursor_pointer()
                                    .children(image.map(|image| img(image).size_full().object_fit(gpui::ObjectFit::Cover)))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.pick_gif(picked.clone(), window, cx)
                                    }))
                            }))
                            .into_any_element()
                    })
                    .collect::<Vec<_>>()
            }),
        )
        .track_scroll(self.emoji_scroll.clone())
        .size_full()
        .into_any_element()
    }
}

impl Emerald {
    /// A sticker as the picker shows it: animated, as in Discord's picker,
    /// by players of its own drawn at the picker's size. Only rows on
    /// screen are drawn, so only those load and play.
    fn render_picker_sticker(&self, sticker: &Sticker) -> AnyElement {
        let Some(url) = sticker.url.as_deref().filter(|_| sticker.lottie && self.animates()) else {
            return self.render_sticker(sticker, STICKER);
        };
        let frame = self.images.picker_animation(url, &*self.net).and_then(|player| {
            self.animating.set(true);
            player.frame(self.animation_clock.elapsed())
        });
        match frame {
            Some(frame) => img(frame).size(zoomed(STICKER)).into_any_element(),
            None => div().size(zoomed(STICKER)).rounded_lg().bg(theme::hover()).into_any_element(),
        }
    }
}

/// The muted line a tab shows when it has nothing to list.
fn empty(text: impl Into<SharedString>) -> AnyElement {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .text_color(theme::muted())
        .child(text.into())
        .into_any_element()
}
