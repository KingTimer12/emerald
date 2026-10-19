//! User profile beside 1:1 DMs and the "Ver Perfil Completo" modal, both
//! fed by `GET /users/{id}/profile`.

use gpui::{Context, Div, FontWeight, MouseButton, SharedString, Window, div, img, prelude::*, px, rgb};

use super::super::{theme, time};
use super::widgets::{self, ButtonKind};
use super::{Emerald, Overlay};
use crate::discord::model::Snowflake;

const PANEL_WIDTH: f32 = 340.;

/// What both profile views show, resolved from the user and profile caches.
struct Card {
    name: SharedString,
    username: SharedString,
    banner: u32,
    pronouns: Option<String>,
    bio: Option<String>,
    /// Badge descriptions with their icon URL.
    badges: Vec<(SharedString, String)>,
    /// Connected accounts as (service, account name).
    accounts: Vec<(String, String)>,
}

impl Emerald {
    fn card(&self, user: Snowflake) -> Card {
        let known = self.people.user(user);
        let profile = self.profiles.get(&user);
        let details = profile.map(|p| &p.user_profile);
        Card {
            name: known.map_or_else(|| user.to_string(), |u| u.display_name().to_owned()).into(),
            username: known.map(|u| u.username.clone()).unwrap_or_default().into(),
            banner: profile
                .and_then(|p| {
                    p.user
                        .banner_color
                        .as_deref()
                        .and_then(|hex| u32::from_str_radix(hex.trim_start_matches('#'), 16).ok())
                        .or(p.user.accent_color)
                })
                // Without a banner color Discord paints a neutral grey.
                .unwrap_or(theme::INTERACTIVE_MUTED),
            pronouns: details.and_then(|d| d.pronouns.clone()).filter(|p| !p.is_empty()),
            bio: details.and_then(|d| d.bio.clone()).filter(|b| !b.trim().is_empty()),
            badges: profile.map_or_else(Vec::new, |p| {
                p.badges
                    .iter()
                    .map(|b| (b.description.clone().into(), b.icon_url()))
                    .collect()
            }),
            accounts: profile.map_or_else(Vec::new, |p| {
                p.connected_accounts
                    .iter()
                    .map(|a| (a.kind.clone(), a.name.clone()))
                    .collect()
            }),
        }
    }

    /// Username followed by the badge icons, each with its description on hover.
    fn name_line(&self, card: &Card, text_size: f32, badge_size: f32) -> Div {
        let mut line = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_1()
            .child(
                div()
                    .mr_1()
                    .text_size(px(text_size))
                    .text_color(rgb(theme::TEXT_DEFAULT))
                    .child(card.username.clone()),
            );
        for (ix, (description, url)) in card.badges.iter().enumerate() {
            let badge = div()
                .id(("badge", ix))
                .flex_none()
                .size(px(badge_size))
                .tooltip(widgets::tooltip(description.clone()));
            line = line.child(match self.avatar_image(url) {
                Some(image) => badge.child(img(image).size_full()),
                None => badge,
            });
        }
        line
    }

    fn avatar_ring(&self, user: Snowflake, card: &Card, size: f32, ring: f32) -> Div {
        div()
            .p(px(ring))
            .rounded_full()
            .bg(rgb(theme::BASE_LOWEST))
            .child(widgets::avatar(
                &card.name,
                user,
                self.user_avatar(user),
                size,
                Some((self.people.presence(user), theme::BASE_LOWEST)),
            ))
    }

    pub(super) fn render_profile_panel(&self, user: Snowflake, cx: &mut Context<Self>) -> impl IntoElement {
        let card = self.card(user);
        const BANNER: f32 = 120.;
        const AVATAR: f32 = 80.;
        const RING: f32 = 6.;

        div()
            .w(px(PANEL_WIDTH))
            .flex_none()
            .h_full()
            .p_2()
            .border_l_1()
            .border_color(theme::border())
            .child(
                div()
                    .relative()
                    .size_full()
                    .flex()
                    .flex_col()
                    .rounded_lg()
                    .overflow_hidden()
                    .border_1()
                    .border_color(theme::border())
                    .bg(rgb(theme::BASE_LOWEST))
                    .child(div().w_full().h(px(BANNER)).flex_none().bg(rgb(card.banner)))
                    .child(
                        // Avatar straddling the banner's bottom edge.
                        div()
                            .absolute()
                            .top(px(BANNER - AVATAR / 2. - RING))
                            .left_4()
                            .child(self.avatar_ring(user, &card, AVATAR, RING)),
                    )
                    .child(
                        div()
                            .id("profile-body")
                            .w_full()
                            .flex_1()
                            .min_h(px(0.))
                            .overflow_y_scroll()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .px_4()
                            .pt(px(AVATAR / 2. + RING + 12.))
                            .pb_4()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_size(px(20.))
                                            .line_height(px(24.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(theme::TEXT_STRONG))
                                            .child(card.name.clone()),
                                    )
                                    .child(self.name_line(&card, 14., 20.)),
                            )
                            .children(card.pronouns.clone().map(|p| {
                                div()
                                    .text_size(px(14.))
                                    .text_color(rgb(theme::TEXT_MUTED))
                                    .child(p)
                            }))
                            .children(card.bio.clone().map(|bio| section("Sobre mim", text(bio))))
                            .child(section(
                                "Membro desde",
                                text(time::short_date(user.unix_secs())),
                            )),
                    )
                    .child(
                        div().w_full().flex_none().p_3().child(
                            widgets::button("full-profile", "Ver Perfil Completo", ButtonKind::Secondary)
                                .w_full()
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.open_profile(user, window, cx)
                                })),
                        ),
                    ),
            )
    }

    pub(super) fn open_profile(&mut self, user: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        self.profile(user);
        self.overlay = Some(Overlay::Profile(user));
        window.focus(&self.focus_handle);
        cx.notify();
    }

    pub(super) fn render_profile_modal(&self, user: Snowflake, cx: &mut Context<Self>) -> impl IntoElement {
        let card = self.card(user);
        const BANNER: f32 = 210.;
        const AVATAR: f32 = 120.;
        const RING: f32 = 8.;

        let accounts = (!card.accounts.is_empty()).then(|| {
            section(
                "Contas conectadas",
                div().flex().flex_wrap().gap_2().children(card.accounts.iter().map(|(kind, name)| {
                    div()
                        .flex()
                        .flex_col()
                        .px_3()
                        .py_2()
                        .rounded_lg()
                        .border_1()
                        .border_color(theme::border())
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(theme::TEXT_MUTED))
                                .child(kind.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(theme::TEXT_STRONG))
                                .child(name.clone()),
                        )
                })),
            )
        });

        // Scrim: clicking outside the card closes it.
        div()
            .id("profile-scrim")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme::scrim())
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.close_overlay(window, cx)),
            )
            .child(
                div()
                    .id("profile-modal")
                    .relative()
                    .w(px(600.))
                    .flex()
                    .flex_col()
                    .rounded_lg()
                    .overflow_hidden()
                    .border_1()
                    .border_color(theme::border())
                    .bg(rgb(theme::BASE_LOWEST))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(div().w_full().h(px(BANNER)).flex_none().bg(rgb(card.banner)))
                    .child(
                        div()
                            .absolute()
                            .top_3()
                            .right_3()
                            .rounded_md()
                            .bg(theme::scrim())
                            .child(widgets::icon_button("close-profile", "✕").on_click(
                                cx.listener(|this, _, window, cx| this.close_overlay(window, cx)),
                            )),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(BANNER - AVATAR / 2. - RING))
                            .left_6()
                            .child(self.avatar_ring(user, &card, AVATAR, RING)),
                    )
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .px_6()
                            .pt(px(AVATAR / 2. + RING + 16.))
                            .pb_6()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_size(px(24.))
                                            .line_height(px(30.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(theme::TEXT_STRONG))
                                            .child(card.name.clone()),
                                    )
                                    .child(self.name_line(&card, 16., 24.))
                                    .children(card.pronouns.clone().map(|p| {
                                        div()
                                            .text_size(px(14.))
                                            .text_color(rgb(theme::TEXT_MUTED))
                                            .child(p)
                                    })),
                            )
                            .child(div().w_full().h(px(1.)).bg(theme::border()))
                            .children(card.bio.clone().map(|bio| section("Sobre mim", text(bio))))
                            .child(section(
                                "Membro desde",
                                text(time::short_date(user.unix_secs())),
                            ))
                            .children(accounts),
                    ),
            )
    }
}

fn section(title: &'static str, body: impl IntoElement) -> Div {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(14.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(theme::TEXT_STRONG))
                .child(title),
        )
        .child(body)
}

fn text(text: String) -> Div {
    div()
        .text_size(px(14.))
        .line_height(px(18.))
        .text_color(rgb(theme::TEXT_DEFAULT))
        .child(text)
}
