//! The member list beside guild channels, fed by `state::MemberList`.

use crate::zoom::zoomed;

mod row;


use std::ops::Range;
use std::time::Duration;

use gpui::{AnyElement, Context, FontWeight, ScrollStrategy, SharedString, div, prelude::*, rgb, uniform_list};

use super::super::theme;
use super::state::MemberList;
use super::{Emerald, widgets};
use emerald_discord::Command;
use super::state::{member_search, members};
use super::activity::activity_text;
use emerald_discord::model::{MemberListItem, Snowflake, User};

/// Pauses typing this long before the search is sent.
const SEARCH_DELAY: Duration = Duration::from_millis(300);

/// What one member row shows, whichever list it comes from.
struct MemberRow<'a> {
    user: &'a User,
    nick: Option<&'a str>,
    roles: &'a [Snowflake],
    /// `None` when presence is unknown, as in search results.
    status: Option<&'a str>,
    activity: Option<String>,
}

const WIDTH: f32 = 256.;
const ROW_HEIGHT: f32 = 44.;
/// Scrolling settles this long before the new ranges are subscribed, so a
/// fling through a huge list sends one op 37 instead of dozens.
const RESUBSCRIBE_DELAY: Duration = Duration::from_millis(250);

impl Emerald {
    /// Subscribes to the open guild channel's member list unless it is
    /// already the one shown; DMs and a hidden list drop it.
    pub(super) fn sync_member_list(&mut self, cx: &mut Context<Self>) {
        let space = self.spaces.current();
        let channel = self.selected_channel.filter(|_| self.settings.show_members && !space.is_dms());
        let Some(mut channel) = channel else {
            self.members.close();
            return;
        };
        // A thread shows its parent channel's list.
        if let Some(parent) = space.channels().iter().find(|c| c.id == channel && c.kind.is_thread()).and_then(|c| c.parent_id) {
            channel = parent;
        }
        let overwrites = space
            .channels()
            .iter()
            .find(|c| c.id == channel)
            .map_or(&[][..], |c| c.permission_overwrites.as_slice());
        // The @everyone role shares the guild's id.
        let everyone = space.info().map_or(0, |guild| guild.role_permissions(space.id));
        let id = members::list_id(everyone, overwrites);
        let other_guild = self.members.current().is_none_or(|old| old.guild != space.id);
        if !self.members.open(space.id, channel, id) {
            return;
        }
        if other_guild {
            self.member_query.update(cx, |input, cx| input.set_text("", cx));
        }
        if let Some(list) = self.members.current() {
            self.net.post(Command::SubscribeGuild {
                guild: list.guild,
                members: Some((channel, list.ranges().to_vec())),
            });
        }
        self.members_scroll.scroll_to_item(0, ScrollStrategy::Top);
    }

    /// The search box changed: after a pause in typing, asks the gateway
    /// for the members matching it.
    pub(super) fn on_member_query(&mut self, cx: &mut Context<Self>) {
        let text = self.member_query.read(cx).text().to_owned();
        let Some(generation) = self.member_search.set_query(&text) else {
            cx.notify();
            return;
        };
        cx.notify();
        let Some(guild) = self.members.current().map(|list| list.guild) else { return };
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SEARCH_DELAY).await;
            this.update(cx, |this, _| {
                if this.member_search.is_current(generation) {
                    this.net.post(Command::SearchMembers {
                        guild,
                        query: this.member_search.query().to_owned(),
                        nonce: member_search::nonce(generation),
                    });
                }
            })
            .ok();
        })
        .detach();
    }

    /// Follows the rows on screen with the subscribed ranges, debounced.
    fn member_rows_visible(&mut self, visible: Range<usize>, cx: &mut Context<Self>) {
        let Some(list) = self.members.current_mut() else { return };
        // uniform_list also renders one row alone to measure the row height;
        // that is not where the user is.
        if visible.len() <= 1 {
            return;
        }
        if list.scrolled(visible).is_none() || std::mem::replace(&mut self.members_resubscribe, true) {
            return;
        }
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(RESUBSCRIBE_DELAY).await;
            this.update(cx, |this, _| {
                this.members_resubscribe = false;
                if let Some(list) = this.members.current() {
                    this.net.post(Command::SubscribeGuild {
                        guild: list.guild,
                        members: Some((list.channel, list.ranges().to_vec())),
                    });
                }
            })
            .ok();
        })
        .detach();
    }

    #[hotpath::measure]
    pub(super) fn render_member_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let searching = self.member_search.is_active();
        let body = if searching {
            self.render_search_results(cx).into_any_element()
        } else {
            let rows = self.members.current().map_or(0, MemberList::len);
            uniform_list(
                "members",
                rows,
                cx.processor(|this, range: Range<usize>, _, cx| {
                    this.member_rows_visible(range.clone(), cx);
                    range.map(|ix| this.render_member_row(ix, cx)).collect::<Vec<_>>()
                }),
            )
            .track_scroll(self.members_scroll.clone())
            .size_full()
            .pb_4()
            .into_any_element()
        };
        div()
            .w(zoomed(WIDTH))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .border_l_1()
            .border_color(theme::border())
            .bg(rgb(theme::base_lower()))
            // Kept, though 2026 has no member search: styled like the header's.
            .child(
                div().w_full().flex_none().p_2().child(
                    div()
                        .h(zoomed(32.))
                        .px_2()
                        .flex()
                        .items_center()
                        .gap_1()
                        .rounded(theme::radius::control())
                        .border_1()
                        .border_color(theme::border_normal())
                        .bg(rgb(theme::surface_high()))
                        .child(div().flex_1().min_w(zoomed(0.)).child(self.member_query.clone()))
                        .child(crate::icons::icon(crate::icons::SEARCH, 16., theme::interactive())),
                ),
            )
            .child(div().w_full().flex_1().min_h(zoomed(0.)).child(body))
    }

    fn render_search_results(&self, cx: &mut Context<Self>) -> AnyElement {
        let results = self.member_search.results().len();
        if results == 0 {
            let text = if self.member_search.is_waiting() {
                t!("members.searching")
            } else {
                t!("members.no_results")
            };
            return div()
                .w_full()
                .p_4()
                .text_size(zoomed(14.))
                .text_color(rgb(theme::text_muted()))
                .child(text)
                .into_any_element();
        }
        uniform_list(
            "member-results",
            results,
            cx.processor(|this, range: Range<usize>, _, cx| {
                range
                    .filter_map(|ix| this.render_search_row(ix, cx))
                    .collect::<Vec<_>>()
            }),
        )
        .size_full()
        .pb_4()
        .into_any_element()
    }
}
