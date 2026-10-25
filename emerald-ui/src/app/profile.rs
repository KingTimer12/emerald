//! User profile beside 1:1 DMs and the "View Full Profile" modal, both
//! fed by `GET /users/{id}/profile`.

mod modal;
mod social;

pub(super) use social::ProfileTab;

use gpui::{Context, Div, FontWeight, MouseButton, SharedString, Window, div, img, prelude::*, px, rgb};

use super::super::{theme, time};
use super::widgets::{self, ButtonKind};
use super::{Emerald, Overlay};
use emerald_discord::model::Snowflake;

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
    /// Their roles in the guild, highest first, as (name, color).
    roles: Vec<(String, u32)>,
    /// When they joined the guild, in Unix seconds.
    joined: Option<i64>,
}

impl Emerald {
    /// The card of `user`, as members of `guild` see them when given.
    fn card(&self, user: Snowflake, guild: Option<Snowflake>) -> Card {
        let known = self.people.user(user);
        let profile = self.profiles.get(&(user, guild));
        // Global badges and accounts come with the guild-less profile.
        let global_profile = self.profiles.get(&(user, None));
        let info = guild.and_then(|id| self.spaces.guild(id));
        let membership = profile.and_then(|p| p.guild_member.as_ref());
        let seen = info.and_then(|info| info.member(user));
        // Guild-only bio and pronouns win over the global ones when set.
        let local = profile.and_then(|p| p.guild_member_profile.as_ref());
        let global = global_profile.or(profile).map(|p| &p.user_profile);
        let pronouns = local
            .and_then(|d| d.pronouns.clone())
            .filter(|p| !p.is_empty())
            .or_else(|| global.and_then(|d| d.pronouns.clone()).filter(|p| !p.is_empty()));
        let bio = local
            .and_then(|d| d.bio.clone())
            .filter(|b| !b.trim().is_empty())
            .or_else(|| global.and_then(|d| d.bio.clone()).filter(|b| !b.trim().is_empty()));
        let nick = membership
            .and_then(|m| m.nick.clone())
            .or_else(|| seen.and_then(|m| m.nick.clone()));
        let role_ids = membership.map(|m| m.roles.as_slice()).or(seen.map(|m| m.roles.as_slice()));
        let roles = info
            .zip(role_ids)
            .map(|(info, ids)| {
                info.roles_of(ids)
                    .into_iter()
                    .map(|r| (r.name.clone(), r.color))
                    .collect()
            })
            .unwrap_or_default();
        let profile = global_profile.or(profile);
        Card {
            name: nick
                .or_else(|| known.map(|u| u.display_name().to_owned()))
                .unwrap_or_else(|| user.to_string())
                .into(),
            roles,
            joined: membership.and_then(|m| m.joined_secs()),
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
            pronouns,
            bio,
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
                self.profile_avatar(user),
                size,
                Some((self.people.presence(user), theme::BASE_LOWEST)),
            ))
    }

    pub(super) fn render_profile_panel(&self, user: Snowflake, cx: &mut Context<Self>) -> impl IntoElement {
        let card = self.card(user, None);
        const BANNER: f32 = 120.;
        // `self.card` names people by their own name; the panel sits by
        // the DM, where a friend goes by the nickname given.
        let card = Card {
            name: self.people.nickname(user).map_or(card.name.clone(), |n| n.to_owned().into()),
            ..card
        };
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
                            .children(card.bio.clone().map(|bio| section(t!("profile.about"), text(bio))))
                            .child(section(
                                t!("profile.member_since"),
                                text(time::short_date(user.unix_secs())),
                            )),
                    )
                    .child(
                        div().w_full().flex_none().p_3().child(
                            widgets::button("full-profile", t!("profile.view_full"), ButtonKind::Secondary)
                                .w_full()
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.open_profile(user, None, window, cx)
                                }))
                                .on_mouse_down(MouseButton::Right, self.on_user_right_click(user, None, None, cx)),
                        ),
                    ),
            )
    }

    pub(super) fn open_profile(
        &mut self,
        user: Snowflake,
        guild: Option<Snowflake>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.profile(user, guild, true);
        // Badges and connected accounts only come with the global profile.
        if guild.is_some() {
            self.profile(user, None, false);
        }
        self.profile_tab = ProfileTab::About;
        if !self.people.is_me(user) {
            self.load_note(user, cx);
        }
        self.overlay = Some(Overlay::Profile { user, guild });
        window.focus(&self.focus_handle);
        cx.notify();
    }
}

/// Discord's join date, plus the guild's when the card is a member's.
fn member_since(user: Snowflake, card: &Card, guild: Option<SharedString>) -> Div {
    let discord = time::short_date(user.unix_secs());
    match (guild, card.joined) {
        (Some(guild), Some(joined)) => div()
            .flex()
            .flex_col()
            .child(text(format!("Discord: {discord}")))
            .child(text(format!("{guild}: {}", time::short_date(joined)))),
        _ => text(discord),
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
