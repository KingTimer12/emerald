//! The "Active Now" column beside the friend list.

use gpui::{AnyElement, Context, FontWeight, SharedString, div, prelude::*, rgb};

use super::super::Emerald;
use super::super::activity::headline;
use super::super::state::active_now::{ActiveCard, active_now};
use super::super::widgets;
use crate::theme;
use crate::zoom::zoomed;
use emerald_discord::model::{Snowflake, relationship_kind};

/// The column's width, and the narrowest window that still shows it.
pub(super) const WIDTH: f32 = 360.;
pub(super) const MIN_WINDOW: f32 = 1200.;
/// Avatars listed on a voice card before "+N".
const SHOWN_IN_VOICE: usize = 3;

impl Emerald {
    /// Where `user` is talking: a voice channel of a server the user shares
    /// with them, or a call in a DM or group.
    fn voice_of(&self, user: Snowflake) -> Option<(Option<Snowflake>, Snowflake)> {
        self.spaces
            .iter()
            .filter(|space| !space.is_dms())
            .find_map(|space| space.voice_channel_of(user).map(|channel| (Some(space.id), channel)))
            .or_else(|| self.dm_calls.channel_of(user).map(|channel| (None, channel)))
    }

    pub(super) fn render_active_now(&self, cx: &mut Context<Self>) -> AnyElement {
        let friends: Vec<Snowflake> = self
            .people
            .relationships()
            .filter(|(_, kind)| *kind == relationship_kind::FRIEND)
            .map(|(id, _)| id)
            .collect();
        let cards = active_now(&friends, |user| self.people.activities(user).to_vec(), |user| self.voice_of(user));
        let body = if cards.is_empty() {
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_1()
                .pt_4()
                .text_center()
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(theme::text_strong()))
                        .child(t!("friends.quiet")),
                )
                .child(div().text_size(zoomed(14.)).text_color(rgb(theme::text_muted())).child(t!("friends.quiet_hint")))
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .gap_3()
                .children(cards.into_iter().map(|card| self.render_active_card(card, cx)))
                .into_any_element()
        };
        div()
            .id("active-now")
            .w(zoomed(WIDTH))
            .flex_none()
            .h_full()
            .overflow_y_scroll()
            .p_4()
            .border_l_1()
            .border_color(theme::border())
            .child(
                div()
                    .mb_4()
                    .text_size(zoomed(20.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(theme::text_strong()))
                    .child(t!("friends.active_now")),
            )
            .child(body)
            .into_any_element()
    }

    fn render_active_card(&self, card: ActiveCard, cx: &mut Context<Self>) -> AnyElement {
        let name = |user: Snowflake| -> SharedString {
            self.people.friend_name(user).map_or_else(|| user.to_string(), str::to_owned).into()
        };
        let (first, title, subtitle, footer) = match card {
            ActiveCard::Voice { guild, channel, users } => {
                let first = users[0];
                let title: SharedString = match users.len() {
                    1 => name(first),
                    n => t!("friends.and_others", name = name(first), count = n - 1).into(),
                };
                let place: SharedString = guild
                    .and_then(|guild| self.spaces.iter().find(|space| space.id == guild))
                    .and_then(|space| space.channels().iter().find(|c| c.id == channel))
                    .and_then(|c| c.name.clone())
                    .unwrap_or_default()
                    .into();
                let more = users.len().saturating_sub(SHOWN_IN_VOICE);
                let footer = div()
                    .mt_2()
                    .pt_2()
                    .border_t_1()
                    .border_color(theme::border_normal())
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(div().flex_1().min_w(zoomed(0.)).truncate().text_size(zoomed(14.)).child(place))
                    .children(users.iter().take(SHOWN_IN_VOICE).map(|user| {
                        widgets::avatar(&name(*user), *user, self.user_avatar(*user), 20., None)
                    }))
                    .when(more > 0, |row| {
                        row.child(div().text_size(zoomed(12.)).text_color(rgb(theme::text_muted())).child(format!("+{more}")))
                    });
                (first, title, SharedString::from(t!("friends.in_voice")), Some(footer))
            }
            ActiveCard::Activity { user, activity } => (user, name(user), SharedString::from(headline(&activity).unwrap_or_else(|| activity.name.clone())), None),
        };
        div()
            .id(("active-card", first.0 as usize))
            .p_3()
            .flex()
            .flex_col()
            .rounded(theme::radius::card())
            .border_1()
            .border_color(theme::border_normal())
            .bg(rgb(theme::base_low()))
            .cursor_pointer()
            .hover(|s| s.bg(theme::hover()))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(widgets::avatar(
                        &title,
                        first,
                        self.user_avatar(first),
                        32.,
                        self.people.status(first).map(|s| (s.as_ref(), theme::base_low())),
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .min_w(zoomed(0.))
                            .child(
                                div()
                                    .truncate()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(theme::text_strong()))
                                    .child(title),
                            )
                            .child(div().truncate().text_size(zoomed(12.)).text_color(rgb(theme::text_muted())).child(subtitle)),
                    ),
            )
            .children(footer)
            .on_click(cx.listener(move |this, _, window, cx| this.open_dm(first, window, cx)))
            .on_mouse_down(gpui::MouseButton::Right, self.on_user_right_click(first, None, None, cx))
            .into_any_element()
    }
}
