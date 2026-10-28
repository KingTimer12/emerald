//! "Select Friends": picks friends for a new group DM, or to add to a DM,
//! as the web client's popout beside the DM list and the DM header does.

use gpui::{AnyElement, Context, Focusable, FontWeight, MouseButton, SharedString, Window, div, prelude::*, rgb};

use super::super::{icons, theme};
use super::widgets::{self, ButtonKind};
use super::{Emerald, Overlay};
use emerald_discord::Command;
use emerald_discord::model::{ChannelKind, Snowflake};
use crate::zoom::zoomed;

/// Discord lets ten people into a group DM, the user included.
pub(super) const GROUP_LIMIT: usize = 10;

pub(super) struct GroupPicker {
    /// The DM to add people to; `None` starts a new conversation.
    channel: Option<Snowflake>,
    picked: Vec<Snowflake>,
}

impl Emerald {
    pub(super) fn open_group_picker(&mut self, channel: Option<Snowflake>, window: &mut Window, cx: &mut Context<Self>) {
        self.place_menu = None;
        self.picker_query.update(cx, |input, cx| input.set_text("", cx));
        window.focus(&self.picker_query.read(cx).focus_handle(cx));
        self.overlay = Some(Overlay::GroupPicker(GroupPicker { channel, picked: Vec::new() }));
        cx.notify();
    }

    /// Who is in the DM already, the user aside.
    fn group_members(&self, channel: Option<Snowflake>) -> Vec<Snowflake> {
        channel
            .and_then(|id| self.spaces.channel(id))
            .map(|c| c.recipient_ids.clone())
            .unwrap_or_default()
    }

    /// Friends matching the search, by name, minus those already in.
    fn pickable_friends(&self, channel: Option<Snowflake>, cx: &Context<Self>) -> Vec<(Snowflake, SharedString)> {
        let query = self.picker_query.read(cx).text().trim().to_lowercase();
        let members = self.group_members(channel);
        let mut friends: Vec<(Snowflake, SharedString)> = self
            .people
            .relationships()
            .filter(|(id, _)| self.people.is_friend(*id) && !members.contains(id))
            .filter_map(|(id, _)| {
                let name = self.people.friend_name(id)?.to_owned();
                let username = self.people.user(id).map(|u| u.username.to_lowercase()).unwrap_or_default();
                (query.is_empty() || name.to_lowercase().contains(&query) || username.contains(&query))
                    .then(|| (id, name.into()))
            })
            .collect();
        friends.sort_by_cached_key(|(_, name)| name.to_lowercase());
        friends
    }

    fn toggle_picked(&mut self, user: Snowflake, cx: &mut Context<Self>) {
        let room = GROUP_LIMIT - 1 - self.group_members(self.picked_channel()).len();
        let Some(Overlay::GroupPicker(picker)) = &mut self.overlay else { return };
        match picker.picked.iter().position(|u| *u == user) {
            Some(ix) => {
                picker.picked.remove(ix);
            }
            None if picker.picked.len() < room => picker.picked.push(user),
            None => {}
        }
        cx.notify();
    }

    fn picked_channel(&self) -> Option<Snowflake> {
        match &self.overlay {
            Some(Overlay::GroupPicker(picker)) => picker.channel,
            _ => None,
        }
    }

    /// One friend from the DM list opens their DM; more start a group. In
    /// a DM they join it.
    fn finish_group_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Overlay::GroupPicker(picker)) = &self.overlay else { return };
        let (channel, picked) = (picker.channel, picker.picked.clone());
        if picked.is_empty() {
            return;
        }
        self.close_overlay(window, cx);
        match (channel, picked.as_slice()) {
            (Some(channel), _) => self.net.post(Command::AddRecipients { channel, users: picked }),
            (None, [one]) => {
                self.open_dm(*one, window, cx);
                true
            }
            (None, _) => self.net.post(Command::CreateGroup { users: picked }),
        };
    }

    pub(super) fn render_group_picker(&self, picker: &GroupPicker, cx: &mut Context<Self>) -> AnyElement {
        let friends = self.pickable_friends(picker.channel, cx);
        let in_group = self.group_members(picker.channel).len();
        let room = (GROUP_LIMIT - 1).saturating_sub(in_group + picker.picked.len());
        let group = picker.channel.and_then(|c| self.spaces.channel(c)).is_some_and(|c| c.kind == ChannelKind::GROUP_DM);
        let action = match (picker.channel, picker.picked.len()) {
            (Some(_), _) if group => t!("group.add"),
            (None, 1) => t!("group.create_dm"),
            _ => t!("group.create"),
        };
        let rows: Vec<AnyElement> = friends
            .into_iter()
            .enumerate()
            .map(|(ix, (id, name))| {
                let picked = picker.picked.contains(&id);
                let username: SharedString = self.people.user(id).map(|u| u.username.clone()).unwrap_or_default().into();
                div()
                    .id(("pick-friend", ix))
                    .h(zoomed(44.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_2()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme::hover()))
                    .child(widgets::avatar(&name, id, self.user_avatar(id), 32., None))
                    .child(
                        div()
                            .flex_1()
                            .min_w(zoomed(0.))
                            .flex()
                            .gap_2()
                            .items_center()
                            .child(div().truncate().font_weight(FontWeight::MEDIUM).text_color(rgb(theme::text_strong())).child(name))
                            .child(div().truncate().text_size(zoomed(14.)).text_color(rgb(theme::text_muted())).child(username)),
                    )
                    .child(checkbox(picked))
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_picked(id, cx)))
                    .into_any_element()
            })
            .collect();
        let empty = rows.is_empty();
        div()
            .id("picker-scrim")
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
                    .id("group-picker")
                    .w(zoomed(440.))
                    .max_h(zoomed(560.))
                    .flex()
                    .flex_col()
                    .rounded_lg()
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
                                    .child(t!("group.select_friends")),
                            )
                            .child(div().text_xs().text_color(theme::muted()).child(t!("group.room", count = room)))
                            .child(div().mt_2().child(self.picker_query.clone())),
                    )
                    .child(
                        div()
                            .id("picker-list")
                            .flex_1()
                            .min_h(zoomed(0.))
                            .overflow_y_scroll()
                            .px_2()
                            .flex()
                            .flex_col()
                            .children(rows)
                            .when(empty, |list| {
                                list.child(div().p_4().text_color(theme::muted()).child(t!("group.no_friends")))
                            }),
                    )
                    .child(
                        div().p_4().flex().child(
                            widgets::button("picker-done", action, ButtonKind::Brand)
                                .w_full()
                                .when(picker.picked.is_empty(), |b| b.opacity(0.5))
                                .on_click(cx.listener(|this, _, window, cx| this.finish_group_picker(window, cx))),
                        ),
                    ),
            )
            .into_any_element()
    }
}

fn checkbox(on: bool) -> gpui::Div {
    div()
        .size(zoomed(22.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .border_1()
        .border_color(if on { rgb(theme::brand()) } else { theme::border() })
        .when(on, |b| b.bg(rgb(theme::brand())).child(icons::icon(icons::CHECK, 16., theme::text_strong())))
}
