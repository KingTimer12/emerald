//! One run of styled text: spoilers, links, mentions, selection and animated emoji painted over it.

use super::*;
use crate::zoom::zoomed;

impl Emerald {
    /// The selection over piece `part` of `message`, if it holds one.
    pub(super) fn selected_range(&self, message: Snowflake, part: usize, text: &str) -> Option<Range<usize>> {
        self.text_selection.as_ref()?.range_in(message, part, text)
    }

    /// One run of styled text with its clicks, painted emoji and selection.
    pub(super) fn render_piece(&self, id: ElementId, rich: Rich, part: usize, options: &Options) -> AnyElement {
        self.render_text(id, rich, part, options, false)
    }

    /// An emoji-only message, drawn large with its note at text size beside.
    pub(super) fn render_jumbo(&self, id: ElementId, rich: Rich, options: &Options) -> AnyElement {
        let text = self.render_text(id, rich, 0, options, true);
        match options.note {
            Some(note) => div()
                .flex()
                .items_end()
                .gap_1()
                .child(text)
                .child(div().pb_1().text_color(rgb(theme::text_muted())).child(note.to_owned()))
                .into_any_element(),
            None => text,
        }
    }

    pub(super) fn render_text(&self, id: ElementId, mut rich: Rich, part: usize, options: &Options, jumbo: bool) -> AnyElement {
        let id = ElementId::NamedChild(Box::new(id), format!("text-{part}").into());
        // Role mentions in the role's color, as the web client tints them.
        let guild = self.spaces.current().info();
        for (range, mention) in &rich.mentions {
            if let Mention::Role(role) = mention
                && let Some(color) = guild.and_then(|g| g.role(*role)).map(|r| r.color).filter(|c| *c != 0)
            {
                rich.spans.push((range.clone(), Style::Tinted(color)));
            }
        }
        let keys: Vec<Option<SpoilerKey>> = (0..rich.spoilers.len())
            .map(|n| {
                options.scope.map(|(message, scope_part)| SpoilerKey {
                    message,
                    part: scope_part,
                    block: part as u32,
                    spoiler: n as u32,
                })
            })
            .collect();
        let hidden: Vec<Range<usize>> = rich
            .spoilers
            .iter()
            .zip(&keys)
            .filter(|(_, key)| key.is_none_or(|key| !self.revealed_spoilers.contains(&key)))
            .map(|(range, _)| range.clone())
            .collect();
        let covered = |range: &Range<usize>| hidden.iter().any(|h| h.start < range.end && range.start < h.end);

        let mut clicks: Vec<(Range<usize>, Click)> = Vec::new();
        if options.clickable {
            for (range, key) in rich.spoilers.iter().zip(&keys) {
                if let Some(key) = key.filter(|key| !self.revealed_spoilers.contains(key)) {
                    clicks.push((range.clone(), Click::Reveal(key)));
                }
            }
            for (range, url) in &rich.links {
                if !covered(range) {
                    let click = match markdown::channel_link(url) {
                        Some((guild, channel, message)) => Click::Link(guild, channel, message),
                        None => Click::Open(url.clone()),
                    };
                    clicks.push((range.clone(), click));
                }
            }
            for (range, mention) in &rich.mentions {
                let click = match mention {
                    Mention::Channel(channel) => Click::Channel(*channel),
                    Mention::User(user) => Click::User(*user),
                    Mention::Role(_) => continue,
                };
                if !covered(range) {
                    clicks.push((range.clone(), click));
                }
            }
        }

        let text: SharedString = std::mem::take(&mut rich.text).into();
        let selectable = options.own.map(|message| Selectable {
            message,
            part,
            view: self.weak.clone(),
        });
        let selected = options.own.and_then(|message| self.selected_range(message, part, &text));
        let styled = StyledText::new(text.clone()).with_highlights(flatten(&rich.spans, &hidden, selected));
        let layout = styled.layout().clone();
        let shown = if clicks.is_empty() {
            styled.into_any_element()
        } else {
            let ranges = clicks.iter().map(|(range, _)| range.clone()).collect();
            let view = self.weak.clone();
            InteractiveText::new(id, styled)
                .on_click(ranges, move |ix, window, cx| match clicks[ix].1.clone() {
                    Click::Open(url) => cx.open_url(&url),
                    click => {
                        view.update(cx, |this, cx| this.on_text_click(click, window, cx)).ok();
                    }
                })
                .into_any_element()
        };
        let layer = selectable.map(|s| select::selection_layer(s, text, layout.clone()));
        let animate = self.animates();
        let emojis: Vec<_> = rich
            .emojis
            .into_iter()
            .filter(|emoji| !covered(&emoji.range))
            .filter_map(|emoji| {
                // gpui draws only a WebP's first frame: moving ones are
                // decoded apart, and show still until they are.
                let moving = (emoji.animated && animate)
                    .then(|| self.images.animated(&animated_custom_url(emoji.id), EMOJI_PIXELS, EMOJI_PIXELS, &*self.net))
                    .flatten();
                let animated = moving.is_some();
                let image = match moving {
                    Some(frames) => ImageSource::Render(frames),
                    None => ImageSource::Image(self.avatar_image(&custom_url(emoji.id))?),
                };
                Some((emoji.range, image, animated))
            })
            .collect();
        if emojis.is_empty() && !jumbo && layer.is_none() {
            return shown;
        }
        if emojis.iter().any(|(_, _, animated)| *animated) {
            self.animating.set(true);
        }
        let elapsed = self.animation_clock.elapsed();
        div()
            .relative()
            .when(jumbo, |text| text.text_size(zoomed(JUMBO * 0.8)).line_height(zoomed(JUMBO + 4.)))
            .children(layer)
            .child(shown)
            .child(
                canvas(
                    move |_, window, cx| {
                        emojis
                            .into_iter()
                            .filter_map(|(range, image, animated)| {
                                let bounds = emoji_bounds(&layout, range, jumbo)?;
                                let image = match image {
                                    ImageSource::Render(frames) => frames,
                                    ImageSource::Image(image) => image.use_render_image(window, cx)?,
                                    _ => return None,
                                };
                                let frame = if animated { frame_at(&image, elapsed) } else { 0 };
                                Some((bounds, image, frame))
                            })
                            .collect::<Vec<_>>()
                    },
                    |_, emojis, window, _| {
                        for (bounds, image, frame) in emojis {
                            if let Err(e) = window.paint_image(bounds, Default::default(), image, frame, false) {
                                tracing::warn!("could not paint an emoji: {e}");
                            }
                        }
                    },
                )
                .absolute()
                .size_full(),
            )
            .into_any_element()
    }

    /// A click on a spoiler, a channel or a user in some text.
    pub(super) fn on_text_click(&mut self, click: Click, window: &mut gpui::Window, cx: &mut gpui::Context<Self>) {
        match click {
            Click::Reveal(key) => {
                if self.revealed_spoilers.len() >= MAX_REVEALED {
                    self.revealed_spoilers.clear();
                }
                self.revealed_spoilers.insert(key);
                if let Some(channel) = self.selected_channel {
                    self.history.redraw(channel, key.message, super::super::super::shown(&self.messages, Some(channel), channel));
                }
                cx.notify();
            }
            Click::Channel(channel) => self.open_channel_link(None, channel, None, window, cx),
            Click::Link(guild, channel, message) => self.open_channel_link(guild, channel, message, window, cx),
            Click::User(user) => {
                let guild = self.spaces.current().info().map(|_| self.spaces.current().id);
                self.open_profile(user, guild, window, cx);
            }
            Click::Open(url) => cx.open_url(&url),
        }
    }
}

/// The frame of an animated image due `elapsed` into the animation clock,
/// following each frame's own delay.
pub(super) fn frame_at(image: &gpui::RenderImage, elapsed: Duration) -> usize {
    let count = image.frame_count();
    if count <= 1 {
        return 0;
    }
    // GIFs that ask for no delay play at the browsers' 100ms.
    let delay = |ix: usize| {
        let delay = Duration::from(image.delay(ix));
        if delay < Duration::from_millis(20) { Duration::from_millis(100) } else { delay }
    };
    let total: Duration = (0..count).map(delay).sum();
    let mut at = Duration::from_nanos((elapsed.as_nanos() % total.as_nanos().max(1)) as u64);
    for ix in 0..count {
        let this = delay(ix);
        if at < this {
            return ix;
        }
        at -= this;
    }
    count - 1
}

/// Where the emoji kept at `range` goes: centred on its room, as tall as
/// Discord draws it beside text of this line height.
pub(super) fn emoji_bounds(layout: &TextLayout, range: Range<usize>, jumbo: bool) -> Option<Bounds<gpui::Pixels>> {
    let start = layout.position_for_index(range.start)?;
    let end = layout.position_for_index(range.end)?;
    let line = layout.line_height();
    let side = if jumbo { zoomed(JUMBO) } else { line * 1.1 };
    // A room split by wrapping shows the emoji where it starts.
    let room = if end.y == start.y { end.x - start.x } else { side };
    let origin = point(start.x + (room - side) / 2., start.y + (line - side) / 2.);
    Some(Bounds::new(origin, size(side, side)))
}

pub(super) fn selection_highlight() -> HighlightStyle {
    HighlightStyle {
        background_color: Some(theme::text_selection().into()),
        ..Default::default()
    }
}
