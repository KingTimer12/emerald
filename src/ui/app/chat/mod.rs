//! The chat column: header, message list with the channel intro and day
//! separators, composer with the typing indicator, and the DM profile panel.

use std::ops::Range;

use gpui::{
    AnyElement, Context, Div, FontStyle, FontWeight, HighlightStyle, InteractiveText, SharedString,
    StrikethroughStyle, StyledText, UnderlineStyle, div, img, list, prelude::*, px, rgb,
};

use super::super::markdown::{self, Mention, Style};
use super::super::{icons, theme, time};
use super::widgets;
use super::{DMS, Emerald, GROUP_WINDOW_SECS, Log, Msg, Reply, TOP};
use crate::discord::model::{Embed, Snowflake, message_kind};

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
        let topic: Option<SharedString> = self
            .selected_channel
            .and_then(|id| self.spaces[self.selected_space].topic(id))
            .map(|t| t.replace('\n', " ").into());

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

        if self.spaces[self.selected_space].id == DMS {
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
                        .child(icons::icon(icon, 44., theme::TEXT_STRONG)),
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

    /// Author name and color: guild nickname and top role color when the
    /// member is known, else the account's display name.
    fn author_style(&self, msg: &Msg) -> (SharedString, u32) {
        let guild = self.spaces[self.selected_space].guild.as_ref();
        let (nick, color) = guild.map_or((None, None), |g| g.member_style(msg.author_id));
        (
            nick.map_or_else(|| msg.author.clone(), |n| n.to_owned().into()),
            color.unwrap_or(theme::TEXT_STRONG),
        )
    }

    fn render_message(&self, log: &Log, ix: usize) -> AnyElement {
        let msg = &log.messages[ix];
        let previous = ix.checked_sub(1).map(|ix| &log.messages[ix]);
        let new_day = previous.is_none_or(|p| p.day != msg.day);
        let (author, author_color) = self.author_style(msg);
        let row = div()
            .id(("message", ix))
            .w_full()
            .hover(|s| s.bg(theme::hover()));

        let message = if let Some(text) = system_text(msg.kind, &author) {
            // System line: arrow in the avatar column, then the event.
            row.flex()
                .items_center()
                .gap_4()
                .px_4()
                .py(px(2.))
                .when(!new_day, |r| r.mt(px(15.)))
                .child(
                    div()
                        .w(px(40.))
                        .flex_none()
                        .flex()
                        .justify_center()
                        .text_color(rgb(theme::ONLINE))
                        .child("→"),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .text_color(theme::muted())
                        .child(text),
                )
                .child(
                    div()
                        .flex_none()
                        .text_xs()
                        .text_color(theme::muted())
                        .child(msg.time.clone()),
                )
                .into_any_element()
        } else if self.settings.compact {
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
                                .text_color(rgb(author_color))
                                .font_weight(FontWeight::MEDIUM)
                                .child(author),
                        )
                        .child(self.message_body(msg, ix)),
                )
                .into_any_element()
        } else if !new_day
            && msg.reply.is_none()
            && previous.is_some_and(|p| {
                p.author_id == msg.author_id
                    && msg.secs - p.secs < GROUP_WINDOW_SECS
                    && system_text(p.kind, "").is_none()
            })
        {
            row.pl(px(72.))
                .pr_4()
                .py(px(2.))
                .child(self.message_body(msg, ix))
                .into_any_element()
        } else {
            // Group start: 17px gap above, outside the hover highlight. A day
            // separator already provides the gap.
            div()
                .w_full()
                .when(!new_day, |d| d.pt(px(15.)))
                .child(
                    row.flex()
                        .flex_col()
                        .px_4()
                        .py(px(2.))
                        .children(msg.reply.as_ref().map(|reply| self.render_reply(reply)))
                        .child(
                            div()
                                .flex()
                                .gap_4()
                                .child(div().mt(px(2.)).child(widgets::avatar(
                                    &author,
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
                                                        .text_color(rgb(author_color))
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .child(author),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(theme::muted())
                                                        .child(msg.time.clone()),
                                                ),
                                        )
                                        .child(self.message_body(msg, ix)),
                                ),
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

    /// Discord's reply line: a curved connector into the avatar column, then
    /// the original author and the first line of what they wrote.
    fn render_reply(&self, reply: &Reply) -> Div {
        let line = div().flex().items_center().gap_1().pl(px(20.)).h(px(22.)).text_size(px(14.));
        let connector = div()
            .flex_none()
            .w(px(32.))
            .h(px(10.))
            .mt(px(8.))
            .mr_1()
            .border_l_2()
            .border_t_2()
            .rounded_tl(px(6.))
            .border_color(rgb(theme::INTERACTIVE_MUTED));
        let Some(content) = &reply.content else {
            return line.child(connector).child(
                div()
                    .italic()
                    .text_color(theme::muted())
                    .child("A mensagem original foi apagada"),
            );
        };
        let guild = self.spaces[self.selected_space].guild.as_ref();
        let (nick, color) = guild.map_or((None, None), |g| g.member_style(reply.author_id));
        let name: SharedString = nick.map_or_else(|| reply.author.clone(), |n| n.to_owned().into());
        line.child(connector)
            .child(widgets::avatar(
                &name,
                reply.author_id,
                self.avatar_image(&reply.avatar_url),
                16.,
                None,
            ))
            .child(
                div()
                    .flex_none()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(color.unwrap_or(theme::TEXT_STRONG)))
                    .child(format!("@{name}")),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .truncate()
                    .text_color(theme::muted())
                    .child(self.rich_text(("reply", 0usize), content, &[], false)),
            )
    }

    /// Message text with markdown and mentions resolved against what this
    /// client knows; links open in the browser.
    fn rich_text(
        &self,
        id: impl Into<gpui::ElementId>,
        content: &str,
        mentions: &[(Snowflake, SharedString)],
        clickable: bool,
    ) -> AnyElement {
        let space = &self.spaces[self.selected_space];
        let guild = space.guild.as_ref();
        let name = |mention: Mention| match mention {
            Mention::User(id) => guild
                .and_then(|g| g.member_style(id).0.map(str::to_owned))
                .or_else(|| mentions.iter().find(|(m, _)| *m == id).map(|(_, n)| n.to_string()))
                .or_else(|| self.users.get(&id).map(|u| u.display_name().to_owned()))
                .unwrap_or_else(|| "usuário-desconhecido".to_owned()),
            Mention::Channel(id) => self
                .spaces
                .iter()
                .find_map(|s| s.channels.iter().find(|c| c.id == id))
                .and_then(|c| c.name.clone())
                .unwrap_or_else(|| "canal-desconhecido".to_owned()),
            Mention::Role(id) => guild
                .and_then(|g| g.roles.iter().find(|r| r.id == id))
                .map_or_else(|| "cargo-desconhecido".to_owned(), |r| r.name.clone()),
        };
        let rich = markdown::parse(content, &name);
        let links = rich.links;
        let styled = StyledText::new(rich.text).with_highlights(flatten(&rich.spans));
        if !clickable || links.is_empty() {
            return styled.into_any_element();
        }
        let ranges = links.iter().map(|(range, _)| range.clone()).collect();
        InteractiveText::new(id, styled)
            .on_click(ranges, move |ix, _, cx| cx.open_url(&links[ix].1))
            .into_any_element()
    }

    fn message_body(&self, msg: &Msg, ix: usize) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .min_w(px(0.))
            .text_color(if msg.pending { theme::muted() } else { theme::text() })
            .when(!msg.content.is_empty(), |body| {
                body.child(self.rich_text(("content", ix), &msg.content, &msg.mentions, true))
            })
            .children(msg.images.iter().map(|(url, w, h)| {
                let frame = div().size_full().rounded_lg().overflow_hidden();
                div()
                    .w(px(*w))
                    .h(px(*h))
                    .child(match self.avatar_image(url) {
                        Some(image) => frame.child(img(image).size_full()),
                        None => frame.bg(theme::hover()),
                    })
            }))
            .children(msg.files.iter().map(|name| {
                div()
                    .max_w(px(432.))
                    .flex()
                    .items_center()
                    .gap_2()
                    .p_3()
                    .rounded_lg()
                    .border_1()
                    .border_color(theme::border())
                    .bg(rgb(theme::BASE_LOW))
                    .child(div().text_size(px(24.)).child("📄"))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .text_color(rgb(theme::LINK))
                            .child(name.clone()),
                    )
            }))
            .children(msg.embeds.iter().enumerate().filter_map(|(n, embed)| self.render_embed(embed, ix, n)))
            .children(msg.stickers.iter().map(|(name, url)| {
                match url.as_ref().and_then(|url| self.avatar_image(url)) {
                    Some(image) => img(image).size(px(STICKER_SIZE)).into_any_element(),
                    // Lottie stickers, and images still downloading: a card
                    // with the sticker's name in its place.
                    None => div()
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

    /// Text parts of a rich embed in a card with its color bar; embeds with
    /// nothing textual (bare image previews) are skipped.
    fn render_embed(&self, embed: &Embed, message: usize, n: usize) -> Option<Div> {
        let provider = embed.provider.as_ref().and_then(|p| p.name.clone());
        let author = embed.author.as_ref().and_then(|a| a.name.clone());
        let footer = embed.footer.as_ref().and_then(|f| f.text.clone());
        if embed.title.is_none() && embed.description.is_none() && author.is_none() {
            return None;
        }
        let small = |text: String, size: f32, color: u32| {
            div().text_size(px(size)).text_color(rgb(color)).child(text)
        };
        let title = embed.title.clone().map(|title| {
            // Embed URLs come from the sender; open only http(s).
            let url = embed.url.as_deref().and_then(markdown::web_url).map(str::to_owned);
            let link = url.is_some();
            let element = div()
                .text_size(px(16.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(if link { theme::LINK } else { theme::TEXT_STRONG }))
                .child(title);
            match url {
                Some(url) => element
                    .id(("embed-title", message * 16 + n))
                    .cursor_pointer()
                    .on_click(move |_, _, cx| cx.open_url(&url))
                    .into_any_element(),
                None => element.into_any_element(),
            }
        });
        Some(
            div()
                .max_w(px(520.))
                .flex()
                .rounded_md()
                .overflow_hidden()
                .bg(rgb(theme::BASE_LOW))
                .border_1()
                .border_color(theme::border())
                .child(div().w(px(4.)).flex_none().bg(rgb(embed.color.unwrap_or(theme::INTERACTIVE_MUTED))))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w(px(0.))
                        .gap_1()
                        .px_3()
                        .py_2()
                        .children(provider.map(|p| small(p, 12., theme::TEXT_MUTED)))
                        .children(author.map(|a| {
                            small(a, 14., theme::TEXT_STRONG).font_weight(FontWeight::SEMIBOLD)
                        }))
                        .children(title)
                        .children(embed.description.as_ref().map(|description| {
                            div()
                                .text_size(px(14.))
                                .line_height(px(18.))
                                .child(self.rich_text(("embed", message * 16 + n), description, &[], true))
                        }))
                        .children(footer.map(|f| small(f, 12., theme::TEXT_MUTED))),
                ),
        )
    }
}

/// One-line text of a system message, or `None` for a regular one.
fn system_text(kind: u8, author: &str) -> Option<String> {
    Some(match kind {
        message_kind::MEMBER_JOIN => format!("{author} entrou no servidor."),
        message_kind::PIN => format!("{author} fixou uma mensagem neste canal."),
        message_kind::BOOST => format!("{author} impulsionou o servidor!"),
        _ => return None,
    })
}

/// gpui wants highlights sorted and disjoint; nested markdown overlaps, so
/// split at every boundary and merge the styles covering each piece.
fn flatten(spans: &[(Range<usize>, Style)]) -> Vec<(Range<usize>, HighlightStyle)> {
    let mut cuts: Vec<usize> = spans.iter().flat_map(|(r, _)| [r.start, r.end]).collect();
    cuts.sort_unstable();
    cuts.dedup();
    cuts.windows(2)
        .filter_map(|pair| {
            let piece = pair[0]..pair[1];
            let mut style = HighlightStyle::default();
            let mut any = false;
            for (range, kind) in spans {
                if range.start <= piece.start && piece.end <= range.end {
                    any = true;
                    style = style.highlight(highlight(*kind));
                }
            }
            any.then_some((piece, style))
        })
        .collect()
}

fn highlight(style: Style) -> HighlightStyle {
    let line = UnderlineStyle {
        thickness: px(1.),
        color: None,
        wavy: false,
    };
    match style {
        Style::Bold => HighlightStyle {
            font_weight: Some(FontWeight::BOLD),
            ..Default::default()
        },
        Style::Italic => HighlightStyle {
            font_style: Some(FontStyle::Italic),
            ..Default::default()
        },
        Style::Underline => HighlightStyle {
            underline: Some(line),
            ..Default::default()
        },
        Style::Strike => HighlightStyle {
            strikethrough: Some(StrikethroughStyle {
                thickness: px(1.),
                color: None,
            }),
            ..Default::default()
        },
        Style::Code | Style::Spoiler => HighlightStyle {
            background_color: Some(theme::code_bg().into()),
            ..Default::default()
        },
        Style::Link => HighlightStyle {
            color: Some(rgb(theme::LINK).into()),
            ..Default::default()
        },
        Style::Mention => HighlightStyle {
            color: Some(rgb(theme::MENTION_TEXT).into()),
            background_color: Some(theme::mention_bg().into()),
            font_weight: Some(FontWeight::MEDIUM),
            ..Default::default()
        },
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
