//! The people in a group DM, beside its chat in place of a server's member
//! list, with the owner marked.

use gpui::{AnyElement, Context, FontWeight, MouseButton, SharedString, div, prelude::*, rgb};

use super::super::theme;
use super::{Emerald, widgets};
use emerald_discord::model::Snowflake;
use crate::zoom::zoomed;

impl Emerald {
    pub(super) fn render_group_members(&self, channel: Snowflake, cx: &mut Context<Self>) -> AnyElement {
        let group = self.spaces.channel(channel);
        let owner = group.and_then(|c| c.owner_id);
        let mut people: Vec<Snowflake> = group.map(|c| c.recipient_ids.clone()).unwrap_or_default();
        people.push(self.people.my_id());
        let mut people: Vec<(Snowflake, SharedString)> = people
            .into_iter()
            .map(|id| (id, self.people.friend_name(id).unwrap_or("?").to_owned().into()))
            .collect();
        people.sort_by_cached_key(|(_, name)| name.to_lowercase());
        let count = people.len();
        let rows = people.into_iter().enumerate().map(|(ix, (id, name))| {
            let status = self.people.presence(id);
            div()
                .id(("group-member", ix))
                .h(zoomed(44.))
                .flex_none()
                .flex()
                .items_center()
                .gap_3()
                .px_2()
                .rounded_lg()
                .cursor_pointer()
                .hover(|s| s.bg(theme::hover()))
                .when(status == "offline", |row| row.opacity(0.3))
                .child(widgets::avatar(&name, id, self.user_avatar(id), 32., Some((status, theme::base_lowest()))))
                .child(
                    div()
                        .flex_1()
                        .min_w(zoomed(0.))
                        .truncate()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(theme::channels_default()))
                        .child(name),
                )
                .when(owner == Some(id), |row| {
                    row.child(
                        div()
                            .id(("group-owner", ix))
                            .flex_none()
                            .text_size(zoomed(14.))
                            .text_color(rgb(theme::mentioned_bar()))
                            .tooltip(widgets::tooltip(t!("group.owner")))
                            .child("♛"),
                    )
                })
                .on_click(cx.listener(move |this, _, window, cx| this.open_profile(id, None, window, cx)))
                .on_mouse_down(MouseButton::Right, self.on_user_right_click(id, None, Some(channel), cx))
        });
        div()
            .id("group-members")
            .w(zoomed(240.))
            .flex_none()
            .h_full()
            .overflow_y_scroll()
            .px_2()
            .pt_4()
            .border_l_1()
            .border_color(theme::border())
            .bg(rgb(theme::base_lowest()))
            .flex()
            .flex_col()
            .child(widgets::label(t!("group.members", count = count)).px_2().mb_2())
            .children(rows)
            .into_any_element()
    }
}
