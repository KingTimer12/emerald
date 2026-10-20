//! The chat column: header, message list with the channel intro and day
//! separators, composer with the typing indicator, and the DM profile panel.

mod list;
mod rich;

use gpui::{AnyElement, Context, FontWeight, HighlightStyle, SharedString, StyledText, div, list, prelude::*, px, rgb};

use super::super::{icons, theme};
use super::{Emerald, widgets};
use super::state::Msg;
use crate::discord::model::Snowflake;

impl Emerald {
    pub(super) fn render_chat(&self, wide: bool, cx: &mut Context<Self>) -> AnyElement {
        if self.friends_open && self.spaces.current().is_dms() {
            return self.render_friends(cx).into_any_element();
        }
        let space = self.spaces.current();
        let row = self.selected_channel.and_then(|id| space.row(id));
        let (icon, title, user) = row.map_or(("", SharedString::default(), None), |row| {
            (row.icon, row.name.clone(), row.user)
        });
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
            .h(px(48.))
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
                    Some((self.people.presence(user), theme::BASE_LOWER)),
                )),
                None if !icon.is_empty() => {
                    header.child(icons::icon(icon, 24., theme::CHANNELS_DEFAULT))
                }
                None => header,
            })
            .child(
                div()
                    .flex_none()
                    .text_color(rgb(theme::TEXT_STRONG))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title.clone()),
            )
            .children(topic.map(|topic| {
                let topic = self.rich_text("topic", &topic, &[], false);
                // Divider and one-line topic, as in the web client's header.
                div()
                    .flex()
                    .flex_1()
                    .min_w(px(0.))
                    .items_center()
                    .gap_2()
                    .child(div().flex_none().w(px(1.)).h(px(24.)).bg(theme::border()))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .text_size(px(14.))
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child(topic),
                    )
            }))
            .when(loading, |d| {
                d.child(
                    div()
                        .text_xs()
                        .text_color(theme::muted())
                        .child("carregando…"),
                )
            })
            // Right-aligned toggle: the DM profile or the guild member list.
            .when(!has_topic, |d| d.child(div().flex_1()))
            .map(|d| match (user, self.selected_channel) {
                (Some(_), _) => d.child(header_toggle(
                    "toggle-profile",
                    icons::PROFILE,
                    self.settings.show_profile,
                    ["Ocultar perfil do usuário", "Mostrar perfil do usuário"],
                    cx.listener(|this, _, _, cx| {
                        this.settings.show_profile = !this.settings.show_profile;
                        this.save_settings(cx);
                    }),
                )),
                (None, Some(_)) if !space.is_dms() => d.child(header_toggle(
                    "toggle-members",
                    icons::FRIENDS,
                    self.settings.show_members,
                    ["Ocultar lista de membros", "Mostrar lista de membros"],
                    cx.listener(|this, _, _, cx| {
                        this.settings.show_members = !this.settings.show_members;
                        this.sync_member_list(cx);
                        this.save_settings(cx);
                    }),
                )),
                _ => d,
            });

        let body = if self.selected_channel.is_none() {
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme::muted())
                .child("Escolha um canal")
                .into_any_element()
        } else {
            list(
                self.messages.clone(),
                cx.processor(|this, ix, _, _| this.render_list_row(ix)),
            )
            .flex_1()
            .pb_4()
            // Discord's 1.375 line height, scaled with the chosen font size.
            .text_size(px(self.settings.font_size))
            .line_height(px((self.settings.font_size * 1.375).round()))
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
                    .text_size(px(14.))
                    .text_color(rgb(theme::FEEDBACK_CRITICAL))
                    .child(error)
            }))
            .child(self.composer.input.clone())
            // Typing indicator in the strip under the input, as on Discord.
            .child(
                div()
                    .h(px(24.))
                    .flex()
                    .items_center()
                    .text_xs()
                    .text_color(rgb(theme::TEXT_DEFAULT))
                    .children(self.typing_text()),
            );

        let column = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .child(body)
            .child(composer);

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .bg(rgb(theme::BASE_LOWER))
            .border_l_1()
            .border_color(theme::border())
            .child(header)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.))
                    .child(column)
                    .when_some(user.filter(|_| wide && self.settings.show_profile), |row, user| {
                        row.child(self.render_profile_panel(user, cx))
                    })
                    .when(wide && self.members.is_some(), |row| row.child(self.render_member_list(cx))),
            )
            .into_any_element()
    }

    /// "Paulo está digitando…", in the web client's pt-BR wording.
    fn typing_text(&self) -> Option<StyledText> {
        let channel = self.selected_channel?;
        let names: Vec<&str> = self
            .typing
            .who(channel)
            .map(|user| self.people.display_name(user).unwrap_or("Alguém"))
            .collect();
        let (text, bold): (String, Vec<std::ops::Range<usize>>) = match names.as_slice() {
            [] => return None,
            [one] => (format!("{one} está digitando…"), vec![0..one.len()]),
            [a, b] => {
                let second = a.len() + 3;
                (
                    format!("{a} e {b} estão digitando…"),
                    vec![0..a.len(), second..second + b.len()],
                )
            }
            _ => ("Várias pessoas estão digitando…".to_owned(), Vec::new()),
        };
        Some(StyledText::new(text).with_highlights(bold.into_iter().map(|range| {
            (
                range,
                HighlightStyle {
                    font_weight: Some(FontWeight::BOLD),
                    ..Default::default()
                },
            )
        })))
    }

    /// Author name and color: guild nickname and top role color when the
    /// member is known, else the account's display name.
    fn member_style(&self, user: Snowflake, fallback: &SharedString) -> (SharedString, Option<u32>) {
        let guild = self.spaces.current().info();
        let (nick, color) = guild.map_or((None, None), |g| g.member_style(user));
        (nick.map_or_else(|| fallback.clone(), |n| n.to_owned().into()), color)
    }

    fn author_style(&self, msg: &Msg) -> (SharedString, u32) {
        let (name, color) = self.member_style(msg.author_id, &msg.author);
        (name, color.unwrap_or(theme::TEXT_STRONG))
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
        .size(px(32.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .cursor_pointer()
        .hover(|s| s.bg(theme::hover()))
        .tooltip(widgets::tooltip(if open { hide } else { show }))
        .child(icons::icon(icon, 24., if open { theme::TEXT_STRONG } else { theme::INTERACTIVE }))
        .on_click(on_click)
}
