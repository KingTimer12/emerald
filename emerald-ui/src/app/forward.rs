//! "Forward To": sends a message on to up to five channels, with an
//! optional message after it, as the web client's modal does. Which
//! channels are listed lives in [`state::forward`](super::state::forward).

use gpui::{AnyElement, Context, Entity, Focusable, FontWeight, MouseButton, Subscription, Window, div, prelude::*, rgb};

use super::super::text_input::TextInput;
use super::super::{icons, theme};
use super::state::forward::{self, Dest, MAX_PICKED};
use super::state::Row;
use super::widgets::{self, ButtonKind};
use super::group_picker::checkbox;
use super::{Emerald, Overlay};
use crate::zoom::zoomed;
use emerald_discord::model::Snowflake;
use emerald_discord::{Command, ForwardOf, ForwardTo, permissions};

/// Slowmode does not hold back people who can manage the channel or its
/// messages, nor those with the newer "Bypass Slowmode".
const BYPASS_SLOWMODE: u64 = permissions::MANAGE_MESSAGES | permissions::MANAGE_CHANNELS | 1 << 52;

pub(super) struct ForwardModal {
    from: ForwardOf,
    query: Entity<TextInput>,
    text: Entity<TextInput>,
    /// Every place it can go, sidebar order.
    all: Vec<Dest>,
    picked: Vec<Snowflake>,
    _subscription: Subscription,
}

impl Emerald {
    pub(super) fn open_forward(&mut self, channel: Snowflake, message: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        self.message_menu = None;
        let guild = self.spaces.iter().find(|s| !s.is_dms() && s.channels().iter().any(|c| c.id == channel)).map(|s| s.id);
        let query = cx.new(|cx| TextInput::new(t!("forward.search"), cx).keep_on_submit().compact());
        let text = cx.new(|cx| TextInput::new(t!("forward.optional"), cx).keep_on_submit());
        let subscription = cx.observe(&query, |_, _, cx| cx.notify());
        window.focus(&query.focus_handle(cx));
        self.overlay = Some(Overlay::Forward(Box::new(ForwardModal {
            from: ForwardOf { guild, channel, message },
            query,
            text,
            all: self.forward_destinations(),
            picked: Vec::new(),
            _subscription: subscription,
        })));
        cx.notify();
    }

    /// Existing DMs and group DMs, and guild channels the user can send
    /// in. Forum and media channels take posts, not messages.
    fn forward_destinations(&self) -> Vec<Dest> {
        let mut all = Vec::new();
        for space in self.spaces.iter() {
            for row in space.rows() {
                let Row::Channel(row) = row else { continue };
                let kind = row.kind;
                if !kind.is_openable() || kind.is_forum() || kind.is_voice() {
                    continue;
                }
                let rights = space.permissions(row.id);
                if !space.is_dms() && rights & permissions::SEND_MESSAGES == 0 {
                    continue;
                }
                let slowmode = self.spaces.channel(row.id).and_then(|c| c.rate_limit_per_user).unwrap_or(0) > 0;
                all.push(Dest {
                    channel: row.id,
                    name: row.name.to_string(),
                    detail: (!space.is_dms()).then(|| space.name.to_string()),
                    icon: row.icon,
                    user: row.user,
                    slowmode: slowmode && rights & BYPASS_SLOWMODE == 0,
                });
            }
        }
        all
    }

    fn toggle_forward(&mut self, channel: Snowflake, cx: &mut Context<Self>) {
        let Some(Overlay::Forward(modal)) = &mut self.overlay else { return };
        forward::toggle(&mut modal.picked, channel);
        cx.notify();
    }

    fn send_forward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Overlay::Forward(modal)) = &self.overlay else { return };
        if modal.picked.is_empty() {
            return;
        }
        let to = modal
            .picked
            .iter()
            .map(|channel| ForwardTo {
                channel: *channel,
                with_text: !modal.all.iter().any(|d| d.channel == *channel && d.slowmode),
            })
            .collect();
        let text = modal.text.read(cx).text().trim().to_owned();
        let command = Command::Forward {
            from: modal.from,
            to,
            text: (!text.is_empty()).then_some(text),
        };
        self.net.post(command);
        self.close_overlay(window, cx);
    }

    /// Answer to a forward: the toast, or which places it missed.
    pub(super) fn on_forwarded(&mut self, failed: Vec<Snowflake>, error: Option<String>, cx: &mut Context<Self>) {
        if failed.is_empty() {
            self.show_toast(t!("forward.done"), Some(icons::FORWARD), cx);
            return;
        }
        let names: Vec<String> = failed
            .iter()
            .map(|c| self.spaces.channel(*c).and_then(|c| c.name.clone()).unwrap_or_else(|| c.to_string()))
            .collect();
        let error = error.unwrap_or_default();
        self.connection.set_message(t!("forward.failed", places = names.join(", "), error = error));
        cx.notify();
    }

    pub(super) fn render_forward_modal(&self, modal: &ForwardModal, cx: &mut Context<Self>) -> AnyElement {
        let query = modal.query.read(cx).text().to_owned();
        let listed = forward::destinations(&query, &modal.picked, self.recent.channels(), modal.from.channel, &modal.all);
        let full = modal.picked.len() >= MAX_PICKED;
        let rows: Vec<AnyElement> = listed
            .into_iter()
            .map(|ix| {
                let dest = &modal.all[ix];
                let picked = modal.picked.contains(&dest.channel);
                let disabled = full && !picked;
                let channel = dest.channel;
                let leading = match dest.user {
                    Some(user) => widgets::avatar(&dest.name, user, self.user_avatar(user), 24., None).into_any_element(),
                    None => icons::icon(dest.icon, 20., theme::channels_default()).into_any_element(),
                };
                div()
                    .id(("forward-dest", ix))
                    .h(zoomed(44.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_2()
                    .rounded(theme::radius::control())
                    .when(disabled, |row| row.opacity(0.5))
                    .when(!disabled, |row| {
                        row.cursor_pointer()
                            .hover(|s| s.bg(theme::hover()))
                            .on_click(cx.listener(move |this, _, _, cx| this.toggle_forward(channel, cx)))
                    })
                    .child(div().size(zoomed(24.)).flex_none().flex().items_center().justify_center().child(leading))
                    .child(
                        div()
                            .flex_1()
                            .min_w(zoomed(0.))
                            .flex()
                            .gap_2()
                            .items_center()
                            .child(div().truncate().font_weight(FontWeight::MEDIUM).text_color(rgb(theme::text_strong())).child(dest.name.clone()))
                            .children(dest.detail.clone().map(|detail| {
                                div().truncate().text_size(zoomed(12.)).text_color(theme::muted()).child(detail)
                            })),
                    )
                    .child(checkbox(picked))
                    .into_any_element()
            })
            .collect();
        let empty = rows.is_empty();
        let subtitle = if full {
            t!("forward.max", count = MAX_PICKED)
        } else {
            t!("forward.subtitle").into()
        };
        let send = if modal.picked.len() > 1 {
            t!("forward.send_count", count = modal.picked.len())
        } else {
            t!("forward.send").into()
        };
        let preview = self
            .history
            .find(modal.from.channel, modal.from.message)
            .map(|(_, msg)| preview_of(&msg.content))
            .unwrap_or_default();
        div()
            .id("forward-scrim")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme::scrim())
            .occlude()
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| this.close_overlay(window, cx)))
            .child(
                div()
                    .id("forward-modal")
                    .w(zoomed(480.))
                    .max_h(zoomed(640.))
                    .flex()
                    .flex_col()
                    .rounded(theme::radius::modal())
                    .overflow_hidden()
                    .border_1()
                    .border_color(theme::border())
                    .bg(rgb(theme::surface_high()))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .p_4()
                            .child(
                                div()
                                    .text_size(zoomed(20.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(theme::text_strong()))
                                    .child(t!("forward.title")),
                            )
                            .child(div().text_size(zoomed(14.)).text_color(theme::muted()).child(subtitle))
                            .child(div().mt_2().child(modal.query.clone())),
                    )
                    .child(
                        div()
                            .id("forward-list")
                            .flex_1()
                            .min_h(zoomed(0.))
                            .overflow_y_scroll()
                            .px_2()
                            .flex()
                            .flex_col()
                            .children(rows)
                            .when(empty, |list| list.child(div().p_4().text_color(theme::muted()).child(t!("forward.none")))),
                    )
                    .child(
                        div()
                            .p_4()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .border_t_1()
                            .border_color(theme::border())
                            .when(!preview.is_empty(), |footer| {
                                footer.child(
                                    div()
                                        .pl_3()
                                        .border_l_2()
                                        .border_color(theme::border_normal())
                                        .text_size(zoomed(14.))
                                        .text_color(theme::muted())
                                        .max_h(zoomed(40.))
                                        .overflow_hidden()
                                        .child(preview),
                                )
                            })
                            .child(
                                div()
                                    .px_3()
                                    .py_2()
                                    .rounded(theme::radius::control())
                                    .bg(rgb(theme::base_lowest()))
                                    .child(modal.text.clone()),
                            )
                            .child(
                                div().flex().justify_end().child(
                                    widgets::button("forward-send", send, ButtonKind::Brand)
                                        .when(modal.picked.is_empty(), |b| b.opacity(0.5))
                                        .on_click(cx.listener(|this, _, window, cx| this.send_forward(window, cx))),
                                ),
                            ),
                    ),
            )
            .into_any_element()
    }
}

/// The message's first two lines, for the preview above the optional text.
fn preview_of(content: &str) -> String {
    let lines: Vec<&str> = content.lines().filter(|line| !line.trim().is_empty()).take(2).collect();
    let text = lines.join("\n");
    match text.char_indices().nth(160) {
        Some((cut, _)) => format!("{}\u{2026}", &text[..cut]),
        None => text,
    }
}
