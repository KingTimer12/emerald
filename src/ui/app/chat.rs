//! The chat column: header, message list with the channel intro and day
//! separators, composer with the typing indicator, and the DM profile panel.

use gpui::{
    AnyElement, Context, Div, FontWeight, HighlightStyle, SharedString, StyledText, div, img,
    list, prelude::*, px, rgb,
};

use super::super::icons;
use super::super::{theme, time};
use super::widgets;
use super::{DMS, Emerald, GROUP_WINDOW_SECS, Log, Msg, TOP};
use crate::discord::model::Snowflake;

const STICKER_SIZE: f32 = 160.;

impl Emerald {
    pub(super) fn render_chat(&self, wide: bool, cx: &mut Context<Self>) -> AnyElement {
        if self.friends_open && self.spaces[self.selected_space].id == DMS {
            return self.render_friends(cx).into_any_element();
        }
        let (icon, title, user) = self
            .selected_channel
            .and_then(|id| self.spaces[self.selected_space].channel(id))
            .unwrap_or(("", SharedString::default(), None));
        let loading = self
            .selected_channel
            .and_then(|id| self.logs.get(&id))
            .is_some_and(|log| log.loading);

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
                    Some((self.presence(user), theme::BASE_LOWER)),
                )),
                None => header.child(
                    div()
                        .text_size(px(22.))
                        .text_color(rgb(theme::CHANNELS_DEFAULT))
                        .child(icon),
                ),
            })
            .child(
                div()
                    .text_color(rgb(theme::TEXT_STRONG))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title.clone()),
            )
            .when(loading, |d| {
                d.child(
                    div()
                        .text_xs()
                        .text_color(theme::muted())
                        .child("carregando…"),
                )
            })
            .when(user.is_some(), |d| {
                let open = self.settings.show_profile;
                d.child(div().flex_1()).child(
                    div()
                        .id("toggle-profile")
                        .size(px(32.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_md()
                        .cursor_pointer()
                        .hover(|s| s.bg(theme::hover()))
                        .tooltip(widgets::tooltip(if open {
                            "Ocultar perfil do usuário"
                        } else {
                            "Mostrar perfil do usuário"
                        }))
                        .child(icons::icon(
                            icons::PROFILE,
                            24.,
                            if open { theme::TEXT_STRONG } else { theme::INTERACTIVE },
                        ))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.settings.show_profile = !this.settings.show_profile;
                            this.save_settings(cx);
                        })),
                )
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
            .children(self.send_error.clone().map(|error| {
                div()
                    .text_size(px(14.))
                    .text_color(rgb(theme::FEEDBACK_CRITICAL))
                    .child(error)
            }))
            .child(self.input.clone())
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
                    }),
            )
            .into_any_element()
    }

    fn presence(&self, user: Snowflake) -> &str {
        self.presences.get(&user).map_or("offline", |s| s.as_ref())
    }

    /// "Paulo está digitando…", in the web client's pt-BR wording.
    fn typing_text(&self) -> Option<StyledText> {
        let channel = self.selected_channel?;
        let names: Vec<String> = self
            .typing
            .get(&channel)?
            .iter()
            .map(|(user, _)| {
                self.users
                    .get(user)
                    .map_or_else(|| "Alguém".to_owned(), |u| u.display_name().to_owned())
            })
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

    /// Row `ix` of the chat list: the intro on top, then the messages.
    fn render_list_row(&self, ix: usize) -> AnyElement {
        let log = self.selected_channel.and_then(|c| self.logs.get(&c));
        if ix < TOP {
            return self.render_intro(log);
        }
        match log {
            Some(log) if ix - TOP < log.messages.len() => self.render_message(log, ix - TOP),
            _ => div().into_any_element(),
        }
    }

    /// Discord's "beginning of the conversation" block, shown once the
    /// history is loaded to its start.
    fn render_intro(&self, log: Option<&Log>) -> AnyElement {
        if !log.is_some_and(|log| log.complete) {
            return div().h(px(1.)).into_any_element();
        }
        let Some((icon, title, user)) = self
            .selected_channel
            .and_then(|id| self.spaces[self.selected_space].channel(id))
        else {
            return div().h(px(1.)).into_any_element();
        };
        let intro = div().w_full().flex().flex_col().px_4().pt_4();
        let heading = |text: SharedString, size: f32| {
            div()
                .mt_2()
                .text_size(px(size))
                .line_height(px((size * 1.25).round()))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(theme::TEXT_STRONG))
                .child(text)
        };

        if icon == "@" {
            let username = user
                .and_then(|user| self.users.get(&user))
                .map(|u| SharedString::from(u.username.clone()));
            let prefix = "Este é o começo do seu histórico de mensagens diretas com ";
            let text = format!("{prefix}{title}.");
            let name = prefix.len()..prefix.len() + title.len();
            intro
                .children(user.map(|user| widgets::avatar(&title, user, self.user_avatar(user), 80., None)))
                .child(heading(title.clone(), 30.))
                .children(username.map(|username| {
                    div()
                        .text_size(px(24.))
                        .line_height(px(28.))
                        .text_color(rgb(theme::TEXT_STRONG))
                        .child(username)
                }))
                .child(
                    div().mt_2().text_color(theme::muted()).child(StyledText::new(text).with_highlights([(
                        name,
                        HighlightStyle {
                            font_weight: Some(FontWeight::BOLD),
                            color: Some(rgb(theme::TEXT_STRONG).into()),
                            ..Default::default()
                        },
                    )])),
                )
                .into_any_element()
        } else {
            intro
                .child(
                    div()
                        .size(px(68.))
                        .rounded_full()
                        .bg(theme::hover())
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(40.))
                        .text_color(rgb(theme::TEXT_STRONG))
                        .child("#"),
                )
                .child(heading(format!("Boas-vindas a #{title}!").into(), 32.))
                .child(
                    div()
                        .mt_1()
                        .text_color(theme::muted())
                        .child(format!("Este é o começo do canal #{title}.")),
                )
                .into_any_element()
        }
    }

    fn render_message(&self, log: &Log, ix: usize) -> AnyElement {
        let msg = &log.messages[ix];
        let previous = ix.checked_sub(1).map(|ix| &log.messages[ix]);
        let new_day = previous.is_none_or(|p| p.day != msg.day);
        let body = self.message_body(msg);
        let row = div()
            .id(("message", ix))
            .hover(|s| s.bg(theme::hover()));

        let message = if self.settings.compact {
            // Compact mode: one line per message, no avatars or group gaps.
            row.flex()
                .gap_2()
                .px_4()
                .py(px(1.))
                .child(
                    div()
                        .flex_none()
                        .text_xs()
                        .text_color(theme::muted())
                        .child(msg.short_time.clone()),
                )
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .min_w(px(0.))
                        .gap_1()
                        .child(
                            div()
                                .flex_none()
                                .text_color(rgb(theme::TEXT_STRONG))
                                .font_weight(FontWeight::MEDIUM)
                                .child(msg.author.clone()),
                        )
                        .child(body),
                )
                .into_any_element()
        } else if !new_day
            && previous.is_some_and(|p| {
                p.author_id == msg.author_id && msg.secs - p.secs < GROUP_WINDOW_SECS
            })
        {
            row.pl(px(72.)).pr_4().py(px(2.)).child(body).into_any_element()
        } else {
            // Group start: 17px gap above, outside the hover highlight. A day
            // separator already provides the gap.
            div()
                .w_full()
                .when(!new_day, |d| d.pt(px(15.)))
                .child(
                    row.flex()
                        .gap_4()
                        .px_4()
                        .py(px(2.))
                        .child(div().mt(px(2.)).child(widgets::avatar(
                            &msg.author,
                            msg.author_id,
                            self.avatar_image(&msg.avatar_url),
                            40.,
                            None,
                        )))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_w(px(0.))
                                .child(
                                    div()
                                        .flex()
                                        .gap(px(6.))
                                        .items_center()
                                        .child(
                                            div()
                                                .text_color(rgb(theme::TEXT_STRONG))
                                                .font_weight(FontWeight::MEDIUM)
                                                .child(msg.author.clone()),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme::muted())
                                                .child(msg.time.clone()),
                                        ),
                                )
                                .child(body),
                        ),
                )
                .into_any_element()
        };

        if !new_day {
            return message;
        }
        div()
            .w_full()
            .flex()
            .flex_col()
            .child(day_separator(time::day_label(msg.secs)))
            .child(message)
            .into_any_element()
    }

    fn message_body(&self, msg: &Msg) -> Div {
        div()
            .flex()
            .flex_col()
            .text_color(if msg.pending { theme::muted() } else { theme::text() })
            .when(!msg.content.is_empty(), |body| body.child(msg.content.clone()))
            .children(msg.stickers.iter().map(|(name, url)| {
                match url.as_ref().and_then(|url| self.avatar_image(url)) {
                    Some(image) => img(image).mt_1().size(px(STICKER_SIZE)).into_any_element(),
                    // Lottie stickers, and images still downloading: a card
                    // with the sticker's name in its place.
                    None => div()
                        .mt_1()
                        .size(px(STICKER_SIZE))
                        .rounded_lg()
                        .bg(theme::hover())
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap_1()
                        .text_size(px(14.))
                        .text_color(theme::muted())
                        .child(div().text_xs().child("Figurinha"))
                        .child(div().font_weight(FontWeight::SEMIBOLD).child(name.clone()))
                        .into_any_element(),
                }
            }))
    }
}

/// "19 de outubro de 2026" between two hairlines.
fn day_separator(label: String) -> Div {
    let line = || div().flex_1().h(px(1.)).bg(theme::border());
    div()
        .w_full()
        .flex()
        .items_center()
        .gap_1()
        .px_4()
        .mt(px(24.))
        .mb(px(8.))
        .child(line())
        .child(
            div()
                .flex_none()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::muted())
                .child(label),
        )
        .child(line())
}
