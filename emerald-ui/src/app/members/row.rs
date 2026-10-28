//! One row of the member list or the member search, with name, tag and activity.

use super::*;
use crate::zoom::zoomed;

impl Emerald {
    pub(super) fn render_search_row(&self, ix: usize, cx: &mut Context<Self>) -> Option<AnyElement> {
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
        Some(div().w_full().h(zoomed(ROW_HEIGHT)).px_2().child(self.render_member(("result", ix), row, cx)).into_any_element())
    }

    pub(super) fn render_member_row(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let row = div().w_full().h(zoomed(ROW_HEIGHT)).px_2();
        let list = self.members.current();
        match list.and_then(|list| list.row(ix)) {
            Some(MemberListItem::Group(group)) => {
                let count = list.and_then(|l| l.group(&group.id)).map_or(group.count, |g| g.count);
                row.flex()
                    .items_end()
                    .pb(zoomed(6.))
                    .px_4()
                    .text_size(zoomed(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::channels_default()))
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
                    activity: presence.and_then(|p| activity_text(&p.activities)),
                };
                row.child(self.render_member(("member", ix), row_data, cx)).into_any_element()
            }
            // Outside the loaded pages: a placeholder until its page syncs.
            None => row
                .flex()
                .items_center()
                .gap_3()
                .px_2()
                .child(div().size(zoomed(32.)).flex_none().rounded_full().bg(theme::hover()))
                .child(div().h(zoomed(12.)).w(zoomed(60. + (ix % 5) as f32 * 16.)).rounded_full().bg(theme::hover()))
                .into_any_element(),
        }
    }

    pub(super) fn render_member(&self, id: (&'static str, usize), member: MemberRow, cx: &mut Context<Self>) -> AnyElement {
        let MemberRow { user: who, nick, roles, status, activity } = member;
        let user = who.id;
        let guild = self.spaces.current().id;
        let name: SharedString = nick.unwrap_or_else(|| who.display_name()).to_owned().into();
        let color = self
            .spaces
            .current()
            .info()
            .and_then(|guild| guild.role_color(roles))
            .unwrap_or(theme::channels_default());

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
                status.map(|status| (status, theme::base_lowest())),
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(zoomed(0.))
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
                            .text_color(rgb(theme::text_muted()))
                            .child(text)
                    })),
            )
            .on_click(cx.listener(move |this, _, window, cx| this.open_profile(user, Some(guild), window, cx)))
            .on_mouse_down(gpui::MouseButton::Right, self.on_user_right_click(user, Some(guild), None, cx))
            .into_any_element()
    }

    /// "Online", "Offline", or the hoisted role's name.
    pub(super) fn group_label(&self, id: &str) -> String {
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
pub(super) fn app_tag() -> impl IntoElement {
    div()
        .flex_none()
        .px(zoomed(4.))
        .rounded(zoomed(3.))
        .bg(rgb(theme::brand()))
        .text_size(zoomed(10.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(theme::text_strong()))
        .child("APP")
}

/// `1234567` as `1.234.567` or `1,234,567`, as the language groups digits.
pub(super) fn thousands(n: u64) -> String {
    group_digits(n, t!("number.group_separator"))
}

pub(super) fn group_digits(n: u64, separator: &str) -> String {
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

    #[test]
    fn counts_use_dots() {
        assert_eq!(group_digits(0, "."), "0");
        assert_eq!(group_digits(999, "."), "999");
        assert_eq!(group_digits(1_234_567, "."), "1.234.567");
    }
}
