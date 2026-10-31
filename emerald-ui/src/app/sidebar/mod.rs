//! The two left columns: the server list and the current space's channel
//! sidebar (friends entry and DMs, or a guild's categories and channels).
//! The server list, with its folders, is in [`servers`].

use gpui::{
    AnyElement, Context, FontWeight, MouseButton, SharedString, Transformation, div, img, prelude::*, radians, rgb,
    uniform_list,
};

use super::super::icons::{self, icon};
use super::super::theme;
use super::place_menu::Target;
use super::state::folders::Item;
use super::state::{ChannelRow, Row, Space};
use emerald_discord::model::Snowflake;
use super::activity::activity_text;
use super::{Emerald, widgets};
use crate::zoom::zoomed;

mod dms;
mod guild_header;
mod servers;


/// The channel sidebar's width; the user and call panels span it and the
/// server list.
pub(in crate::app) const SIDEBAR_WIDTH: f32 = 240.;
/// The server list's width.
pub(in crate::app) const SERVER_LIST_WIDTH: f32 = 72.;

impl Emerald {
    #[hotpath::measure]
    pub(super) fn render_channels(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let space = self.spaces.current();
        let dms = space.is_dms();
        let header = div()
            .h(zoomed(48.))
            .flex_none()
            .flex()
            .items_center()
            .border_b_1()
            .border_color(theme::border());
        let header = if dms {
            // A button that opens the quick switcher, as on discord.com.
            header
                .px_2()
                .child(
                    div()
                        .id("find-conversation")
                        .w_full()
                        .h(zoomed(32.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(theme::radius::control())
                        .border_1()
                        .border_color(theme::border())
                        .bg(rgb(theme::base_low()))
                        .text_size(zoomed(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(theme::text_muted()))
                        .cursor_pointer()
                        .hover(|s| s.text_color(rgb(theme::text_default())))
                        .child(t!("dm.search_placeholder"))
                        .on_click(cx.listener(|this, _, window, cx| this.open_switcher(window, cx))),
                )
                .into_any_element()
        } else {
            self.render_guild_header(cx)
        };
        let visible = self.visible_rows(cx);
        let clearance = self.panel_clearance();

        div()
            .flex()
            .flex_col()
            .w(zoomed(SIDEBAR_WIDTH))
            .h_full()
            .flex_none()
            .child(header)
            .child(
                uniform_list(
                    "channels",
                    visible.len(),
                    cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                        range
                            .map(|ix| this.render_channel_row(visible[ix], cx))
                            .collect::<Vec<_>>()
                    }),
                )
                .flex_1()
                .pt_2()
                .pb(zoomed(clearance)),
            )
    }

    /// One sidebar row. The outer box spans the list width so hover and
    /// selection backgrounds fill the row, not just its text.
    fn render_channel_row(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        // uniform_list needs one height per list: DM rows fit a 32px avatar.
        let height = if self.spaces.current().is_dms() { 42. } else { 34. };
        div()
            .w_full()
            .h(zoomed(height))
            .px_2()
            .py(zoomed(1.))
            .child(self.render_channel_row_body(ix, cx))
            .into_any_element()
    }

    fn render_channel_row_body(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let row = div().id(ix).size_full().px_2().flex().items_center();
        match &self.spaces.current().rows()[ix] {
            Row::Friends => {
                let selected = self.friends_open;
                let pending = self.people.incoming_requests();
                let color = if selected { theme::text_strong() } else { theme::channels_default() };
                row.group("friends-row")
                    .gap_3()
                    .rounded(theme::radius::control())
                    .cursor_pointer()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(if selected { theme::text_strong() } else { theme::channels_default() }))
                    .when(selected, |r| r.bg(theme::selected()))
                    .when(!selected, |r| {
                        r.hover(|s| s.bg(theme::hover()).text_color(rgb(theme::text_strong())))
                    })
                    // Icon in a 32px slot so it lines up with the DM avatars.
                    .child(
                        div()
                            .flex_none()
                            .w(zoomed(32.))
                            .flex()
                            .justify_center()
                            .child(icon(icons::FRIENDS, 24., color).group_hover("friends-row", |s| {
                                s.text_color(rgb(theme::text_strong()))
                            })),
                    )
                    .child(div().flex_1().child(t!("friends.title")))
                    .when(pending > 0, |r| r.child(widgets::badge(pending)))
                    .on_click(cx.listener(|this, _, _, cx| this.show_friends(cx)))
                    .into_any_element()
            }
            Row::Requests => self.render_requests_row(row, cx),
            Row::Voice(voice) => self.render_voice_row(row, voice, cx),
            Row::Category { id, title } => {
                let category = *id;
                let guild = self.spaces.current().id;
                let collapsed = category.is_some_and(|c| self.notify.is_collapsed(guild, c));
                let group: SharedString = format!("category-{ix}").into();
                row.group(group.clone())
                    .items_end()
                    .gap_1()
                    .pb(zoomed(4.))
                    // "Direct Messages" is a 14px medium heading in 2026;
                    // server categories stay 12px semibold.
                    .text_size(zoomed(if category.is_none() { 14. } else { 12. }))
                    .font_weight(if category.is_none() { FontWeight::MEDIUM } else { FontWeight::SEMIBOLD })
                    .text_color(rgb(theme::channels_default()))
                    .hover(|s| s.text_color(rgb(theme::text_default())))
                    .child(div().truncate().child(title.clone()))
                    .when(category.is_none() && self.spaces.current().is_dms(), |row| row.child(self.new_dm_button(cx)))
                    .when_some(category, |row, category| {
                        // Chevron after the name, pointing right when collapsed.
                        row.cursor_pointer()
                            .child(
                                div().pb(zoomed(1.)).child(
                                    icon(icons::CHEVRON, 12., theme::channels_default())
                                        .group_hover(group, |s| s.text_color(rgb(theme::text_default())))
                                        .when(collapsed, |chevron| {
                                            chevron.with_transformation(Transformation::rotate(radians(
                                                -std::f32::consts::FRAC_PI_2,
                                            )))
                                        }),
                                ),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| this.toggle_category(guild, category, cx)))
                            .on_mouse_down(
                                MouseButton::Right,
                                cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                                    let target = Target::Channel {
                                        guild: Some(guild),
                                        channel: category,
                                        category: true,
                                        thread: false,
                                    };
                                    this.open_place_menu(target, super::widgets::popover::Anchor::pointer(event.position), cx)
                                }),
                            )
                    })
                    .into_any_element()
            }
            Row::Channel(ChannelRow {
                id,
                kind,
                icon: glyph,
                name,
                user,
                picture,
                members,
                ..
            }) => {
                let id = *id;
                let user = *user;
                let glyph = *glyph;
                let opens = kind.is_openable();
                let thread = kind.is_thread();
                let selected = self.selected_channel == Some(id) && !self.friends_open && !self.requests_open;
                let space = self.spaces.current();
                let guild = (!space.is_dms()).then_some(space.id);
                let muted = self.notify.muted(&self.place_of(guild, id), crate::time::now_secs());
                // Muted channels show no unread state, only mentions.
                let unread = self.reads.is_unread(id) && !muted;
                let mentions = self.reads.mentions(id);
                let forbidden = self.history.is_forbidden(id);
                let (color, icon_color) = if forbidden || (muted && !selected) {
                    (theme::interactive_muted(), theme::interactive_muted())
                } else if selected || unread {
                    (theme::text_strong(), theme::channels_default())
                } else {
                    (theme::channels_default(), theme::channels_default())
                };
                row.relative()
                    .gap(zoomed(6.))
                    .rounded(theme::radius::control())
                    .cursor_pointer()
                    .font_weight(if unread { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
                    .text_color(rgb(color))
                    .when(selected, |r| r.bg(theme::selected()))
                    .when(!selected && !forbidden, |r| {
                        r.hover(|s| s.bg(theme::hover()).text_color(rgb(theme::text_strong())))
                    })
                    .when(unread && !selected, |r| {
                        // Unread marker hugging the sidebar's left edge.
                        r.child(
                            div()
                                .absolute()
                                .left(zoomed(-8.))
                                .top(zoomed(13.))
                                .w(zoomed(4.))
                                .h(zoomed(8.))
                                .rounded_r(theme::radius::pill())
                                .bg(rgb(theme::text_strong())),
                        )
                    })
                    .map(|r| match user {
                        // Threads hang under their channel, without an icon,
                        // from a line like the web client's.
                        _ if thread => r.w_auto().ml(zoomed(26.)).text_size(zoomed(15.)).child(
                            div()
                                .absolute()
                                .left(zoomed(-14.))
                                .top(zoomed(-6.))
                                .w(zoomed(12.))
                                .h(zoomed(22.))
                                .border_l_1()
                                .border_b_1()
                                .rounded_bl(theme::radius::control())
                                .border_color(rgb(theme::interactive_muted())),
                        ),
                        Some(user) => r.gap_3().child(widgets::avatar(
                            name,
                            user,
                            self.user_avatar(user),
                            32.,
                            Some((self.people.presence(user), theme::base_lowest())),
                        )),
                        None => match picture {
                            Some(url) => r.gap_3().child(self.group_picture(url)),
                            None => r.child(icon(glyph, 20., icon_color)),
                        },
                    })
                    .child(match members {
                        // Group DMs say how many are in them under the name.
                        Some(count) => div()
                            .flex_1()
                            .min_w(zoomed(0.))
                            .flex()
                            .flex_col()
                            .child(div().truncate().line_height(zoomed(18.)).child(name.clone()))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::NORMAL)
                                    .text_color(rgb(theme::channels_default()))
                                    .child(t!("group.member_count", count = count)),
                            ),
                        // A friend's custom status or game, under their name.
                        None => match user.and_then(|u| activity_text(self.people.activities(u))) {
                            Some(activity) => div()
                                .flex_1()
                                .min_w(zoomed(0.))
                                .flex()
                                .flex_col()
                                .child(div().truncate().line_height(zoomed(18.)).child(name.clone()))
                                .child(
                                    div()
                                        .truncate()
                                        .text_xs()
                                        .font_weight(FontWeight::NORMAL)
                                        .text_color(rgb(theme::channels_default()))
                                        .child(activity),
                                ),
                            None => div().flex_1().min_w(zoomed(0.)).truncate().child(name.clone()),
                        },
                    })
                    .when(mentions > 0 && !selected, |r| r.child(widgets::badge(mentions as usize)))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                            let target = Target::Channel {
                                guild,
                                channel: id,
                                category: false,
                                thread,
                            };
                            this.open_place_menu(target, super::widgets::popover::Anchor::pointer(event.position), cx)
                        }),
                    )
                    .map(|r| {
                        if opens {
                            r.on_click(cx.listener(move |this, _, window, cx| {
                                this.select_channel(id, window, cx)
                            }))
                        } else if kind.is_voice() && let Some(guild) = guild {
                            r.on_click(cx.listener(move |this, _, _, cx| this.join_voice(guild, id, cx)))
                        } else {
                            // Forums and the like are listed only.
                            r.cursor_default()
                                .tooltip(widgets::tooltip(t!("common.unsupported")))
                        }
                    })
                    .into_any_element()
            }
        }
    }
}
