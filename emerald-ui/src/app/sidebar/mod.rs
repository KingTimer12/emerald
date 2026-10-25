//! The two left columns: the server list and the current space's channel
//! sidebar (friends entry and DMs, or a guild's categories and channels).
//! The server list, with its folders, is in [`servers`].

use gpui::{
    AnyElement, Context, FontWeight, MouseButton, SharedString, Transformation, div, img, prelude::*, px, radians, rgb,
    uniform_list,
};

use super::super::icons::{self, icon};
use super::super::theme;
use super::place_menu::Target;
use super::state::folders::Item;
use super::state::{ChannelRow, Row, Space};
use emerald_discord::model::Snowflake;
use super::{Emerald, widgets};

/// Height of the floating user panel plus its margins; lists pad by this.
pub(super) const PANEL_CLEARANCE: f32 = 72.;

mod dms;
mod servers;

impl Emerald {
    pub(super) fn render_channels(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let space = self.spaces.current();
        let dms = space.is_dms();
        let header = div()
            .h(px(48.))
            .flex_none()
            .flex()
            .items_center()
            .border_b_1()
            .border_color(theme::border());
        let header = if dms {
            // Search field in place of a title, like the web client.
            header.px_2().child(self.dm_search.clone())
        } else {
            header
                .px_4()
                .gap_1()
                .text_color(rgb(theme::TEXT_STRONG))
                .font_weight(FontWeight::SEMIBOLD)
                .child(div().flex_1().min_w(px(0.)).truncate().child(space.name.clone()))
                .child(icon(icons::CHEVRON, 18., theme::INTERACTIVE))
        };
        let visible = self.visible_rows(cx);
        let searching = dms && visible.is_empty() && !self.dm_search.read(cx).text().trim().is_empty();

        div()
            .flex()
            .flex_col()
            .w(px(240.))
            .h_full()
            .flex_none()
            .child(header)
            .when(searching, |sidebar| {
                sidebar.child(
                    div()
                        .p_4()
                        .text_size(px(14.))
                        .text_color(theme::muted())
                        .child(t!("dm.no_results")),
                )
            })
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
                .pb(px(PANEL_CLEARANCE)),
            )
    }

    /// One sidebar row. The outer box spans the list width so hover and
    /// selection backgrounds fill the row, not just its text.
    fn render_channel_row(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        // uniform_list needs one height per list: DM rows fit a 32px avatar.
        let height = if self.spaces.current().is_dms() { 42. } else { 34. };
        div()
            .w_full()
            .h(px(height))
            .px_2()
            .py(px(1.))
            .child(self.render_channel_row_body(ix, cx))
            .into_any_element()
    }

    fn render_channel_row_body(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let row = div().id(ix).size_full().px_2().flex().items_center();
        match &self.spaces.current().rows()[ix] {
            Row::Friends => {
                let selected = self.friends_open;
                let pending = self.people.incoming_requests();
                let color = if selected { theme::TEXT_STRONG } else { theme::CHANNELS_DEFAULT };
                row.group("friends-row")
                    .gap_3()
                    .rounded_lg()
                    .cursor_pointer()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(if selected { theme::TEXT_STRONG } else { theme::CHANNELS_DEFAULT }))
                    .when(selected, |r| r.bg(theme::selected()))
                    .when(!selected, |r| {
                        r.hover(|s| s.bg(theme::hover()).text_color(rgb(theme::TEXT_STRONG)))
                    })
                    // Icon in a 32px slot so it lines up with the DM avatars.
                    .child(
                        div()
                            .flex_none()
                            .w(px(32.))
                            .flex()
                            .justify_center()
                            .child(icon(icons::FRIENDS, 24., color).group_hover("friends-row", |s| {
                                s.text_color(rgb(theme::TEXT_STRONG))
                            })),
                    )
                    .child(div().flex_1().child(t!("friends.title")))
                    .when(pending > 0, |r| r.child(widgets::badge(pending)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.friends_open = true;
                        this.requests_open = false;
                        cx.notify();
                    }))
                    .into_any_element()
            }
            Row::Requests => self.render_requests_row(row, cx),
            Row::Category { id, title } => {
                let category = *id;
                let guild = self.spaces.current().id;
                let collapsed = category.is_some_and(|c| self.notify.is_collapsed(guild, c));
                let group: SharedString = format!("category-{ix}").into();
                row.group(group.clone())
                    .items_end()
                    .gap_1()
                    .pb(px(4.))
                    .text_size(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::CHANNELS_DEFAULT))
                    .hover(|s| s.text_color(rgb(theme::TEXT_DEFAULT)))
                    .child(div().truncate().child(title.clone()))
                    .when(category.is_none() && self.spaces.current().is_dms(), |row| row.child(self.new_dm_button(cx)))
                    .when_some(category, |row, category| {
                        // Chevron after the name, pointing right when collapsed.
                        row.cursor_pointer()
                            .child(
                                div().pb(px(1.)).child(
                                    icon(icons::CHEVRON, 12., theme::CHANNELS_DEFAULT)
                                        .group_hover(group, |s| s.text_color(rgb(theme::TEXT_DEFAULT)))
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
                                    this.open_place_menu(target, event.position, cx)
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
                    (theme::INTERACTIVE_MUTED, theme::INTERACTIVE_MUTED)
                } else if selected || unread {
                    (theme::TEXT_STRONG, theme::CHANNELS_DEFAULT)
                } else {
                    (theme::CHANNELS_DEFAULT, theme::CHANNELS_DEFAULT)
                };
                row.relative()
                    .gap(px(6.))
                    .rounded_lg()
                    .cursor_pointer()
                    .font_weight(if unread { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
                    .text_color(rgb(color))
                    .when(selected, |r| r.bg(theme::selected()))
                    .when(!selected && !forbidden, |r| {
                        r.hover(|s| s.bg(theme::hover()).text_color(rgb(theme::TEXT_STRONG)))
                    })
                    .when(unread && !selected, |r| {
                        // Unread marker hugging the sidebar's left edge.
                        r.child(
                            div()
                                .absolute()
                                .left(px(-8.))
                                .top(px(13.))
                                .w(px(4.))
                                .h(px(8.))
                                .rounded_r(px(4.))
                                .bg(rgb(theme::TEXT_STRONG)),
                        )
                    })
                    .map(|r| match user {
                        // Threads hang under their channel, without an icon,
                        // from a line like the web client's.
                        _ if thread => r.w_auto().ml(px(26.)).text_size(px(15.)).child(
                            div()
                                .absolute()
                                .left(px(-14.))
                                .top(px(-6.))
                                .w(px(12.))
                                .h(px(22.))
                                .border_l_1()
                                .border_b_1()
                                .rounded_bl(px(8.))
                                .border_color(rgb(theme::INTERACTIVE_MUTED)),
                        ),
                        Some(user) => r.gap_3().child(widgets::avatar(
                            name,
                            user,
                            self.user_avatar(user),
                            32.,
                            Some((self.people.presence(user), theme::BASE_LOWEST)),
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
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .child(div().truncate().line_height(px(18.)).child(name.clone()))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::NORMAL)
                                    .text_color(rgb(theme::CHANNELS_DEFAULT))
                                    .child(t!("group.member_count", count = count)),
                            ),
                        None => div().flex_1().min_w(px(0.)).truncate().child(name.clone()),
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
                            this.open_place_menu(target, event.position, cx)
                        }),
                    )
                    .map(|r| {
                        if opens {
                            r.on_click(cx.listener(move |this, _, window, cx| {
                                this.select_channel(id, window, cx)
                            }))
                        } else {
                            // Voice, stage and forum channels are listed only.
                            r.cursor_default()
                                .tooltip(widgets::tooltip(t!("common.unsupported")))
                        }
                    })
                    .into_any_element()
            }
        }
    }
}
