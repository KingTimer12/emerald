//! One row of the member list or the member search, with name, tag and activity.

use super::*;
use crate::zoom::zoomed;
use gpui::{ImageSource, ObjectFit, img, linear_color_stop, linear_gradient, relative, rgba};
use emerald_discord::model::Nameplate;
use super::super::super::time;

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
                    .text_size(zoomed(13.))
                    .font_weight(FontWeight::MEDIUM)
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
        let avatar = widgets::avatar(
            &name,
            user,
            self.avatar_image(&who.avatar_url()),
            32.,
            status.map(|status| (status, theme::base_lowest())),
        );
        // Cosmetics play on the row under the pointer, as on Discord.
        let moving = self.hovered_member == Some(user);
        let nameplate = who.collectibles.as_ref().and_then(|c| c.nameplate.as_ref());
        let plate_image = nameplate.and_then(|plate| self.nameplate_image(plate, moving));
        let plate_color = nameplate.and_then(|plate| plate.color(!theme::is_light()));

        div()
            .id(id)
            .relative()
            .size_full()
            .flex()
            .items_center()
            .gap_3()
            .px_2()
            .rounded(theme::radius::control())
            .overflow_hidden()
            .cursor_pointer()
            // A Nitro nameplate: its palette fading in from the left, as
            // the web client's gradient does, and its image toward the end.
            .when_some(plate_color, |row, color| {
                row.bg(linear_gradient(
                    90.,
                    linear_color_stop(rgba(color << 8), 0.),
                    linear_color_stop(rgba((color << 8) | 0x33), 1.),
                ))
            })
            .when_some(plate_image, |row, image| {
                row.child(
                    img(image.clone())
                        .id(widgets::frames_id("nameplate", &image))
                        .absolute()
                        .top_0()
                        .right_0()
                        .h_full()
                        .w(relative(0.6))
                        .object_fit(ObjectFit::Cover),
                )
            })
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                if *hovered {
                    this.hovered_member = Some(user);
                } else if this.hovered_member == Some(user) {
                    this.hovered_member = None;
                }
                cx.notify();
            }))
            .hover(|s| s.bg(theme::hover()))
            // Offline members are drawn faded, as on Discord.
            .when(status == Some("offline"), |row| row.opacity(0.3))
            .child(widgets::decorated(avatar, self.decoration(who, 32., moving), 32.))
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
            .on_click({
                // The list keeps its own copy of the people in it; the
                // profile looks them up among the known ones.
                let who = who.clone();
                cx.listener(move |this, _, window, cx| {
                    this.people.remember_new(&who);
                    this.open_profile(user, Some(guild), window, cx)
                })
            })
            .on_mouse_down(gpui::MouseButton::Right, self.on_user_right_click(user, Some(guild), None, cx))
            .into_any_element()
    }

    /// "Online", "Offline", or the hoisted role's name.
    pub(super) fn group_label(&self, id: &str) -> String {
        let role = id
            .parse()
            .ok()
            .and_then(|role| self.spaces.current().info()?.role(Snowflake(role)))
            .map(|role| role.name.as_str());
        group_title(id, role)
    }
}

/// A member group's heading: the role's own name as written (2026 no longer
/// shouts it in capitals), or the built-in Online and Offline groups.
fn group_title(id: &str, role_name: Option<&str>) -> String {
    match id {
        "online" => t!("members.online").to_owned(),
        "offline" => t!("members.offline").to_owned(),
        _ => role_name.unwrap_or("?").to_owned(),
    }
}

/// Discord's tag for bots and apps.
pub(super) fn app_tag() -> impl IntoElement {
    div()
        .flex_none()
        .px(zoomed(4.))
        .rounded(theme::radius::tag())
        .bg(rgb(theme::brand()))
        .text_size(zoomed(10.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(theme::on_color()))
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

impl Emerald {
    /// A Nitro nameplate's image, moving when `moving` and the window
    /// animates, at the row's size; `None` until downloaded.
    fn nameplate_image(&self, plate: &Nameplate, moving: bool) -> Option<ImageSource> {
        let now = time::now_secs();
        let frames = (moving && self.animates())
            .then(|| plate.url(now, true))
            .flatten()
            .and_then(|url| self.images.animated(&url, WIDTH as u32 * 2, ROW_HEIGHT as u32 * 2, &*self.net));
        match frames {
            Some(frames) => Some(ImageSource::Render(frames)),
            None => self.avatar_image(&plate.url(now, false)?).map(ImageSource::Image),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_groups_keep_their_case() {
        assert_eq!(group_title("123", Some("Classe S")), "Classe S");
        assert_eq!(group_title("online", None), t!("members.online"));
        assert_eq!(group_title("456", None), "?");
    }

    #[test]
    fn counts_use_dots() {
        assert_eq!(group_digits(0, "."), "0");
        assert_eq!(group_digits(999, "."), "999");
        assert_eq!(group_digits(1_234_567, "."), "1.234.567");
    }
}
