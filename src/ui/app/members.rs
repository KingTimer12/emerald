//! The member list beside guild channels, fed by `state::MemberList`.

use std::ops::Range;
use std::time::Duration;

use gpui::{AnyElement, Context, FontWeight, ScrollStrategy, SharedString, div, prelude::*, px, rgb, uniform_list};

use super::super::theme;
use super::state::MemberList;
use super::{Emerald, widgets};
use crate::discord::Command;
use crate::discord::model::{ListMember, ListPresence, MemberListItem, Snowflake, activity_kind};

const WIDTH: f32 = 240.;
const ROW_HEIGHT: f32 = 44.;
/// Scrolling settles this long before the new ranges are subscribed, so a
/// fling through a huge list sends one op 37 instead of dozens.
const RESUBSCRIBE_DELAY: Duration = Duration::from_millis(250);

impl Emerald {
    /// Subscribes to the open guild channel's member list unless it is
    /// already the one shown; DMs and a hidden list drop it.
    pub(super) fn sync_member_list(&mut self) {
        let space = self.spaces.current();
        let channel = self.selected_channel.filter(|_| self.settings.show_members && !space.is_dms());
        let Some(channel) = channel else {
            self.members = None;
            return;
        };
        if self.members.as_ref().is_some_and(|list| list.channel == channel) {
            return;
        }
        let overwrites = space
            .channels()
            .iter()
            .find(|c| c.id == channel)
            .map_or(&[][..], |c| c.permission_overwrites.as_slice());
        let list = MemberList::new(space.id, channel, overwrites);
        self.net.post(Command::SubscribeGuild {
            guild: list.guild,
            members: Some((channel, list.ranges().to_vec())),
        });
        self.members = Some(list);
        self.members_scroll.scroll_to_item(0, ScrollStrategy::Top);
    }

    /// Follows the rows on screen with the subscribed ranges, debounced.
    fn member_rows_visible(&mut self, visible: Range<usize>, cx: &mut Context<Self>) {
        let Some(list) = &mut self.members else { return };
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
                if let Some(list) = &this.members {
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
        let rows = self.members.as_ref().map_or(0, MemberList::len);
        div()
            .w(px(WIDTH))
            .flex_none()
            .h_full()
            .border_l_1()
            .border_color(theme::border())
            .bg(rgb(theme::BASE_LOWEST))
            .child(
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
                .pt_2()
                .pb_4(),
            )
    }

    fn render_member_row(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let row = div().w_full().h(px(ROW_HEIGHT)).px_2();
        let list = self.members.as_ref();
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
            Some(MemberListItem::Member(member)) => row.child(self.render_member(ix, member, cx)).into_any_element(),
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

    fn render_member(&self, ix: usize, member: &ListMember, cx: &mut Context<Self>) -> AnyElement {
        let user = member.user.id;
        let name: SharedString = member
            .nick
            .clone()
            .unwrap_or_else(|| member.user.display_name().to_owned())
            .into();
        let status = member
            .presence
            .as_ref()
            .and_then(|p| p.status.as_deref())
            .unwrap_or("offline");
        let color = self
            .spaces
            .current()
            .info()
            .and_then(|guild| guild.role_color(&member.roles))
            .unwrap_or(theme::CHANNELS_DEFAULT);
        let activity = member.presence.as_ref().and_then(activity_text);

        div()
            .id(("member", ix))
            .size_full()
            .flex()
            .items_center()
            .gap_3()
            .px_2()
            .rounded_lg()
            .cursor_pointer()
            .hover(|s| s.bg(theme::hover()))
            // Offline members are drawn faded, as on Discord.
            .when(status == "offline", |row| row.opacity(0.3))
            .child(widgets::avatar(
                &name,
                user,
                self.avatar_image(&member.user.avatar_url()),
                32.,
                Some((status, theme::BASE_LOWEST)),
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
                            .when(member.user.bot, |line| line.child(app_tag())),
                    )
                    .children(activity.map(|text| {
                        div()
                            .truncate()
                            .text_xs()
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child(text)
                    })),
            )
            .on_click(cx.listener(move |this, _, window, cx| this.open_profile(user, window, cx)))
            .into_any_element()
    }

    /// "Online", "Offline", or the hoisted role's name.
    fn group_label(&self, id: &str) -> String {
        let label = match id {
            "online" => "Online".to_owned(),
            "offline" => "Offline".to_owned(),
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
/// doing, in the pt-BR web client's wording.
fn activity_text(presence: &ListPresence) -> Option<String> {
    let custom = presence.activities.iter().find_map(|a| {
        (a.kind == activity_kind::CUSTOM)
            .then(|| a.state.clone())
            .flatten()
            .filter(|s| !s.trim().is_empty())
    });
    custom.or_else(|| {
        presence.activities.iter().find_map(|a| {
            let verb = match a.kind {
                activity_kind::PLAYING => "Jogando",
                activity_kind::STREAMING => "Transmitindo",
                activity_kind::LISTENING => "Ouvindo",
                activity_kind::WATCHING => "Assistindo",
                activity_kind::COMPETING => "Competindo em",
                _ => return None,
            };
            Some(format!("{verb} {}", a.name))
        })
    })
}

/// `1234567` as `1.234.567`.
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (ix, digit) in digits.chars().enumerate() {
        if ix > 0 && (digits.len() - ix).is_multiple_of(3) {
            out.push('.');
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
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_234_567), "1.234.567");
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
            Some("Ouvindo Spotify".into())
        );
    }
}
