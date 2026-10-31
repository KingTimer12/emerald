//! The chat column: header, message list with the channel intro and day
//! separators, composer with the typing indicator, and the DM profile panel.

use crate::zoom::zoomed;

mod actions;
pub(super) use actions::bulk::Selection;
mod forum;
mod jump;
mod expressions;
mod list;
mod media;
mod picker;
mod poll;
mod poll_form;
mod reactions;
mod rich;
mod select;
mod threads;
mod typing;

pub(super) use actions::{Editing, MessageMenu, Replying};
pub(super) use forum::Forum;
pub(super) use jump::{Jump, JumpTarget};
pub(super) use poll_form::NewPoll;
pub(super) use list::{day_separator, spoiler_pill};
pub(super) use picker::ReactionPicker;
pub(super) use rich::SpoilerKey;
pub(super) use select::TextSelection;
pub(super) use threads::{NewPost, NewThread};

use gpui::{
    AnyElement, Context, ExternalPaths, FontWeight, SharedString, div, list, prelude::*,
    rgb,
};

use super::super::{icons, lottie, theme};
use super::{Emerald, MEMBERS_MIN_WINDOW, PROFILE_MIN_WINDOW, widgets};
use super::state::Msg;
use emerald_discord::model::Snowflake;

impl Emerald {
    /// The chat column and, when the `width` of the window leaves room, the
    /// profile or member list beside it.
    #[hotpath::measure]
    pub(super) fn render_chat(&self, width: f32, cx: &mut Context<Self>) -> AnyElement {
        let wide = width >= PROFILE_MIN_WINDOW;
        let members_fit = width >= MEMBERS_MIN_WINDOW;
        if self.friends_open && self.spaces.current().is_dms() {
            let friends = self.render_friends(width, cx).into_any_element();
            return friends;
        }
        if self.requests_open && self.spaces.current().is_dms() {
            let requests = self.render_requests(cx);
            return requests;
        }
        let space = self.spaces.current();
        let row = self.selected_channel.and_then(|id| space.row(id));
        let opened = self.selected_channel.and_then(|id| space.channels().iter().find(|c| c.id == id));
        let (icon, title, user) = match (row, opened) {
            (Some(row), _) => (row.icon, row.name.clone(), row.user),
            // Forum posts have no sidebar row.
            (None, Some(channel)) if channel.kind.is_thread() => {
                (icons::THREAD, channel.name.clone().unwrap_or_default().into(), None)
            }
            // Nor have message requests.
            (None, Some(channel)) if channel.kind.is_private() => {
                let title = space.label(channel.id).cloned().unwrap_or_else(|| self.people.dm_name(channel).into());
                (icons::FRIENDS, title, channel.recipient())
            }
            _ => ("", SharedString::default(), None),
        };
        let forum = opened.filter(|c| c.kind.is_forum()).map(|c| c.id);
        let group = opened.filter(|c| c.kind == emerald_discord::model::ChannelKind::GROUP_DM).map(|c| c.id);
        let dm = opened.filter(|c| c.kind.is_private() && !c.is_message_request).map(|c| c.id);
        // A thread names its parent first, as the web client's header does.
        let parent: Option<SharedString> = opened
            .filter(|c| c.kind.is_thread())
            .and_then(|c| c.parent_id)
            .and_then(|p| space.channels().iter().find(|c| c.id == p))
            .and_then(|p| p.name.clone())
            .map(Into::into);
        let loading = self
            .selected_channel
            .and_then(|id| self.history.get(id))
            .is_some_and(|log| log.loading);
        let topic: Option<SharedString> = self
            .selected_channel
            .and_then(|id| space.topic(id))
            .map(|t| t.replace('\n', " ").into());

        let has_topic = topic.is_some();
        let header = div()
            .h(zoomed(48.))
            .flex_none()
            .flex()
            .items_center()
            .gap_2()
            .px_4()
            .border_b_1()
            .border_color(theme::border())
            .map(|header| match user {
                Some(user) => header.child(widgets::avatar(
                    &title,
                    user,
                    self.user_avatar(user),
                    24.,
                    Some((self.people.presence(user), theme::base_lower())),
                )),
                None if !icon.is_empty() => {
                    header.child(icons::icon(icon, 24., theme::channels_default()))
                }
                None => header,
            })
            .children(parent.map(|parent| {
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .flex_none()
                    .text_color(rgb(theme::text_muted()))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(parent)
                    .child(icons::icon(icons::CHEVRON, 12., theme::text_muted()).with_transformation(
                        gpui::Transformation::rotate(gpui::radians(-std::f32::consts::FRAC_PI_2)),
                    ))
            }))
            .child(
                div()
                    .flex_none()
                    .text_color(rgb(theme::text_strong()))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title.clone()),
            )
            .children(topic.map(|topic| {
                let topic = self.rich_line("topic", &topic, &[]);
                // A dot, then the topic on one line, as in the 2026 header.
                div()
                    .flex()
                    .flex_1()
                    .min_w(zoomed(0.))
                    .items_center()
                    .gap_2()
                    .child(div().flex_none().text_color(rgb(theme::text_muted())).child("•"))
                    .child(
                        div()
                            .flex_1()
                            .min_w(zoomed(0.))
                            .truncate()
                            .text_size(zoomed(14.))
                            .text_color(rgb(theme::text_muted()))
                            .child(topic),
                    )
            }))
            .when(loading, |d| {
                d.child(
                    div()
                        .text_xs()
                        .text_color(theme::muted())
                        .child(t!("common.loading")),
                )
            })
            // Right-aligned toggle: the DM profile or the guild member list.
            .when(!has_topic, |d| d.child(div().flex_1()))
            .children(self.render_membership_button(cx))
            .children(dm.and_then(|dm| self.render_call_button(dm, cx)))
            .when_some(dm, |d, dm| {
                d.child(
                    widgets::svg_button("add-to-dm", icons::USER_ADD)
                        .tooltip(widgets::tooltip(t!("group.add_friends")))
                        .on_click(cx.listener(move |this, _, window, cx| this.open_group_picker(Some(dm), window, cx))),
                )
            })
            .child({
                let toggle = match (user, self.selected_channel) {
                (Some(_), _) => Some(header_toggle(
                    "toggle-profile",
                    icons::PROFILE,
                    self.settings.show_profile,
                    [t!("chat.hide_profile"), t!("chat.show_profile")],
                    cx.listener(|this, _, _, cx| {
                        this.settings.show_profile = !this.settings.show_profile;
                        this.save_settings(cx);
                    }),
                ).into_any_element()),
                (None, Some(_)) if !space.is_dms() || group.is_some() => Some(header_toggle(
                    "toggle-members",
                    icons::FRIENDS,
                    self.settings.show_members,
                    [t!("chat.hide_members"), t!("chat.show_members")],
                    cx.listener(|this, _, _, cx| {
                        this.settings.show_members = !this.settings.show_members;
                        this.sync_member_list(cx);
                        this.save_settings(cx);
                    }),
                ).into_any_element()),
                _ => None,
                };
                // The toggle sits between the pins and the search box.
                self.render_header_tools(self.selected_channel, toggle, cx)
            });

        let body = if self.selected_channel.is_none() {
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme::muted())
                .child(t!("chat.pick_channel"))
                .into_any_element()
        } else if let Some(forum) = forum {
            self.render_forum(forum, cx)
        } else {
            let messages = list(
                self.messages.clone(),
                cx.processor(|this, ix, _, cx| {
                    let row = this.render_list_row(ix, cx);
                    this.row_drawn(ix, cx);
                    if this.animating.take() {
                        this.next_animation_frame(cx);
                    }
                    row
                }),
            )
            .size_full()
            // Room for the typing line, so it never hides the last message.
            .pb_6()
            // Discord's 1.375 line height, scaled with the chosen font size.
            .text_size(zoomed(self.settings.font_size))
            .line_height(zoomed((self.settings.font_size * 1.375).round()));
            div()
                .relative()
                .flex_1()
                .min_h(zoomed(0.))
                .child(messages)
                // Under the bars, so "jump to present" covers it when shown.
                .children(self.render_typing())
                .children(self.render_unread_bar(cx))
                .children(self.render_present_bar(cx))
                .children(self.render_dm_call_bar(cx))
                .into_any_element()
        };

        let composer = div()
            .flex_none()
            .flex()
            .flex_col()
            .gap_1()
            .px_4()
            .children(self.composer.error.clone().map(|error| {
                div()
                    .text_size(zoomed(14.))
                    .text_color(rgb(theme::feedback_critical()))
                    .child(error)
            }))
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .children(self.render_plus_menu(cx))
                    .children(self.render_suggestions(cx))
                    .children(self.render_command_hint(cx))
                    .children(self.render_reply_bar(cx))
                    .children(self.render_bulk_bar(cx))
                    .children(self.render_attachments(true, cx))
                    .child(
                        // The + stays by the first row as the box grows.
                        div()
                            .flex()
                            .items_start()
                            .gap_1()
                            .children(self.render_plus_button(cx))
                            .child(div().flex_1().min_w(zoomed(0.)).child(self.composer.input.clone()))
                            .children(self.render_emoji_button(cx)),
                    ),
            )
            // The box ends where the user panel does, 8px above the window's
            // edge; the typing line sits above it, over the messages.
            .pb_2();

        let column = div()
            .id("chat-column")
            .flex()
            .flex_col()
            .flex_1()
            .min_w(zoomed(0.))
            .h_full()
            .drag_over::<ExternalPaths>(|style, _, _, _| style.bg(theme::hover()))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| this.on_drop_files(paths, cx)))
            .child(body)
            // Posts are made from the forum's own form, not the message box;
            // a message request is answered before it can be replied to.
            .map(|column| match self.selected_channel.and_then(|c| self.render_request_bar(c, cx)) {
                Some(bar) => column.child(bar),
                None if forum.is_none() => column.child(composer),
                None => column,
            });

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(zoomed(0.))
            .h_full()
            .bg(rgb(theme::base_lower()))
            .border_l_1()
            .border_color(theme::border())
            .child(header)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(zoomed(0.))
                    .child(column)
                    .map(|row| match self.render_panel(cx) {
                        // A panel takes the place of the profile or member list.
                        Some(panel) => row.child(panel),
                        None => row
                            .when_some(user.filter(|_| wide && self.settings.show_profile), |row, user| {
                                row.child(self.render_profile_panel(user, cx))
                            })
                            .when(members_fit && self.members.current().is_some(), |row| {
                                row.child(self.render_member_list(cx))
                            })
                            .when_some(group.filter(|_| members_fit && self.settings.show_members), |row, group| {
                                row.child(self.render_group_members(group, cx))
                            }),
                    }),
            )
            .into_any_element()
    }

    /// Redraws when the next sticker frame is due, on the animation clock
    /// so frames keep an even pace, freeing the frames that were replaced.
    /// Runs only while a row with an animation, or a profile effect with
    /// layers still to come, is drawn.
    pub(super) fn next_animation_frame(&mut self, cx: &mut Context<Self>) {
        if std::mem::replace(&mut self.frame_pending, true) {
            return;
        }
        let elapsed = self.animation_clock.elapsed();
        let wait = lottie::Player::next_tick(elapsed).saturating_sub(elapsed);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            this.update(cx, |this, cx| {
                this.frame_pending = false;
                for image in this.images.take_retired() {
                    cx.drop_image(image, None);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Author name and color: guild nickname and top role color when the
    /// member is known, else the account's display name.
    pub(super) fn member_style(&self, user: Snowflake, fallback: &SharedString) -> (SharedString, Option<u32>) {
        let guild = self.spaces.current().info();
        let (nick, color) = guild.map_or((None, None), |g| g.member_style(user));
        (nick.map_or_else(|| fallback.clone(), |n| n.to_owned().into()), color)
    }

    pub(super) fn author_style(&self, msg: &Msg) -> (SharedString, u32) {
        let (name, color) = self.member_style(msg.author_id, &msg.author);
        (name, color.unwrap_or(theme::text_strong()))
    }
}

/// A 32px icon button in the chat header that shows or hides a side panel.
fn header_toggle(
    id: &'static str,
    icon: &'static str,
    open: bool,
    [hide, show]: [&'static str; 2],
    on_click: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .size(zoomed(32.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme::radius::control())
        .cursor_pointer()
        .hover(|s| s.bg(theme::hover()))
        .tooltip(widgets::tooltip(if open { hide } else { show }))
        .child(icons::icon(icon, 24., if open { theme::text_strong() } else { theme::interactive() }))
        .on_click(on_click)
}
