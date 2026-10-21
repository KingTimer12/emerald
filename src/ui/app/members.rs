//! The member list beside guild channels, fed by `state::MemberList`.

use std::ops::Range;
use std::time::Duration;

use gpui::{AnyElement, Context, FontWeight, ScrollStrategy, SharedString, div, prelude::*, px, rgb, uniform_list};

use super::super::theme;
use super::state::MemberList;
use super::{Emerald, widgets};
use crate::discord::Command;
use super::state::{member_search, members};
use crate::discord::model::{ListPresence, MemberListItem, Snowflake, User, activity_kind};

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

const WIDTH: f32 = 240.;
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
            .w(px(WIDTH))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .border_l_1()
            .border_color(theme::border())
            .bg(rgb(theme::BASE_LOWEST))
            .child(div().w_full().flex_none().p_2().child(self.member_query.clone()))
            .child(div().w_full().flex_1().min_h(px(0.)).child(body))
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
                .text_size(px(14.))
                .text_color(rgb(theme::TEXT_MUTED))
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

    fn render_search_row(&self, ix: usize, cx: &mut Context<Self>) -> Option<AnyElement> {
        let id = *self.member_search.results().get(ix)?;
        let user = self.people.user(id)?;
        let known = self.spaces.current().info().and_then(|guild| guild.member(id));
        let row = MemberRow {
            user,
            nick: known.and_then(|m| m.nick.as_deref()),
            roles: known.map_or(&[][..], |m| m.roles.as_slice()),
            status: None,
            activity: None,
        };
        Some(div().w_full().h(px(ROW_HEIGHT)).px_2().child(self.render_member(("result", ix), row, cx)).into_any_element())
    }

    fn render_member_row(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let row = div().w_full().h(px(ROW_HEIGHT)).px_2();
        let list = self.members.current();
        match list.and_then(|list| list.row(ix)) {
            Some(MemberListItem::Group(group)) => {
                let count = list.and_then(|l| l.group(&group.id)).map_or(group.count, |g| g.count);
                row.flex()
                    .items_end()
                    .pb(px(6.))
                    .px_4()
                    .text_size(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::CHANNELS_DEFAULT))
                    .child(
                        div()
                            .truncate()
                            .child(format!("{} — {}", self.group_label(&group.id), thousands(count))),
                    )
                    .into_any_element()
            }
            Some(MemberListItem::Member(member)) => {
                let presence = member.presence.as_ref();
                let row_data = MemberRow {
                    user: &member.user,
                    nick: member.nick.as_deref(),
                    roles: &member.roles,
                    status: Some(presence.and_then(|p| p.status.as_deref()).unwrap_or("offline")),
                    activity: presence.and_then(activity_text),
                };
                row.child(self.render_member(("member", ix), row_data, cx)).into_any_element()
            }
            // Outside the loaded pages: a placeholder until its page syncs.
            None => row
                .flex()
                .items_center()
                .gap_3()
                .px_2()
                .child(div().size(px(32.)).flex_none().rounded_full().bg(theme::hover()))
                .child(div().h(px(12.)).w(px(60. + (ix % 5) as f32 * 16.)).rounded_full().bg(theme::hover()))
                .into_any_element(),
        }
    }

    fn render_member(&self, id: (&'static str, usize), member: MemberRow, cx: &mut Context<Self>) -> AnyElement {
        let MemberRow { user: who, nick, roles, status, activity } = member;
        let user = who.id;
        let guild = self.spaces.current().id;
        let name: SharedString = nick.unwrap_or_else(|| who.display_name()).to_owned().into();
        let color = self
            .spaces
            .current()
            .info()
            .and_then(|guild| guild.role_color(roles))
            .unwrap_or(theme::CHANNELS_DEFAULT);

        div()
            .id(id)
            .size_full()
            .flex()
            .items_center()
            .gap_3()
            .px_2()
            .rounded_lg()
            .cursor_pointer()
            .hover(|s| s.bg(theme::hover()))
            // Offline members are drawn faded, as on Discord.
            .when(status == Some("offline"), |row| row.opacity(0.3))
            .child(widgets::avatar(
                &name,
                user,
                self.avatar_image(&who.avatar_url()),
                32.,
                status.map(|status| (status, theme::BASE_LOWEST)),
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(0.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .truncate()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(color))
                                    .child(name),
                            )
                            .when(who.bot, |line| line.child(app_tag())),
                    )
                    .children(activity.map(|text| {
                        div()
                            .truncate()
                            .text_xs()
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child(text)
                    })),
            )
            .on_click(cx.listener(move |this, _, window, cx| this.open_profile(user, Some(guild), window, cx)))
            .into_any_element()
    }

    /// "Online", "Offline", or the hoisted role's name.
    fn group_label(&self, id: &str) -> String {
        let label = match id {
            "online" => t!("members.online").to_owned(),
            "offline" => t!("members.offline").to_owned(),
            role => role
                .parse()
                .ok()
                .and_then(|role| self.spaces.current().info()?.role(Snowflake(role)))
                .map_or_else(|| "?".to_owned(), |role| role.name.clone()),
        };
        label.to_uppercase()
    }
}

/// Discord's tag for bots and apps.
fn app_tag() -> impl IntoElement {
    div()
        .flex_none()
        .px(px(4.))
        .rounded(px(3.))
        .bg(rgb(theme::BRAND))
        .text_size(px(10.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(theme::TEXT_STRONG))
        .child("APP")
}

/// The line under a member's name: their custom status, else what they are
/// doing, in the web client's wording.
fn activity_text(presence: &ListPresence) -> Option<String> {
    let custom = presence.activities.iter().find_map(|a| {
        (a.kind == activity_kind::CUSTOM)
            .then(|| a.state.clone())
            .flatten()
            .filter(|s| !s.trim().is_empty())
    });
    custom.or_else(|| {
        presence.activities.iter().find_map(|a| {
            let name = &a.name;
            Some(match a.kind {
                activity_kind::PLAYING => t!("activity.playing", name = name),
                activity_kind::STREAMING => t!("activity.streaming", name = name),
                activity_kind::LISTENING => t!("activity.listening", name = name),
                activity_kind::WATCHING => t!("activity.watching", name = name),
                activity_kind::COMPETING => t!("activity.competing", name = name),
                _ => return None,
            })
        })
    })
}

/// `1234567` as `1.234.567` or `1,234,567`, as the language groups digits.
fn thousands(n: u64) -> String {
    group_digits(n, t!("number.group_separator"))
}

fn group_digits(n: u64, separator: &str) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (ix, digit) in digits.chars().enumerate() {
        if ix > 0 && (digits.len() - ix).is_multiple_of(3) {
            out.push_str(separator);
        }
        out.push(digit);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discord::model::Activity;

    #[test]
    fn counts_use_dots() {
        assert_eq!(group_digits(0, "."), "0");
        assert_eq!(group_digits(999, "."), "999");
        assert_eq!(group_digits(1_234_567, "."), "1.234.567");
    }

    #[test]
    fn custom_status_wins_over_games() {
        let activity = |kind, name: &str, state: Option<&str>| Activity {
            kind,
            name: name.into(),
            state: state.map(Into::into),
        };
        let presence = |activities| ListPresence {
            status: Some("online".into()),
            activities,
        };
        assert_eq!(
            activity_text(&presence(vec![
                activity(activity_kind::PLAYING, "VALORANT", None),
                activity(activity_kind::CUSTOM, "Custom Status", Some("ranked")),
            ])),
            Some("ranked".into())
        );
        assert_eq!(
            activity_text(&presence(vec![activity(activity_kind::LISTENING, "Spotify", None)])),
            Some("Listening to Spotify".into())
        );
    }
}
