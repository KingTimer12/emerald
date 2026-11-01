//! The full profile, opened over the window from a card or a menu, as in
//! Discord 2026: the person on the left (banner, avatar, name, actions, bio
//! and what the user knows of them), and on the right the Board and what
//! both share, in tabs.

use super::*;
use crate::icons;
use crate::zoom::zoomed;
use emerald_discord::model::relationship_kind;

const WIDTH: f32 = 960.;
const HEIGHT: f32 = 800.;
const LEFT: f32 = 400.;
const BANNER: f32 = 140.;
const AVATAR: f32 = 120.;
const RING: f32 = 8.;

impl Emerald {
    pub(in super::super) fn render_profile_modal(
        &self,
        user: Snowflake,
        guild: Option<Snowflake>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let card = self.card(user, guild);
        let left = self
            .themed(
                div()
                    .id("profile-left")
                    .relative()
                    .w(zoomed(LEFT))
                    .flex_none()
                    .h_full()
                    .flex()
                    .flex_col()
                    .overflow_hidden(),
                card.theme.as_ref(),
            )
            .child(self.banner(&card, BANNER))
            .child(
                div()
                    .absolute()
                    .top(zoomed(BANNER - AVATAR / 2. - RING))
                    .left(zoomed(24.))
                    .child(self.avatar_ring(user, &card, AVATAR, RING)),
            )
            .child(
                div()
                    .id("profile-left-scroll")
                    .relative()
                    .flex_1()
                    .min_h(zoomed(0.))
                    .overflow_y_scroll()
                    .child(self.profile_person(user, guild, &card, cx)),
            )
            .children(self.effect(&card, LEFT));
        let right = div()
            .flex_1()
            .min_w(zoomed(0.))
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(theme::base_lowest()))
            .child(self.profile_tabs(user, guild, cx))
            .child(
                div()
                    .id("profile-right-scroll")
                    .flex_1()
                    .min_h(zoomed(0.))
                    .overflow_y_scroll()
                    .p_4()
                    .child(self.profile_tab_body(user, guild, cx)),
            );
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
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| this.close_overlay(window, cx)))
            .child(
                div()
                    .id("profile-modal")
                    .relative()
                    .w(zoomed(WIDTH))
                    .h(zoomed(HEIGHT))
                    .max_w(gpui::relative(0.92))
                    .max_h(gpui::relative(0.92))
                    .flex()
                    .rounded(theme::radius::modal())
                    .overflow_hidden()
                    .border_1()
                    .border_color(theme::border())
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(left)
                    .child(right)
                    .child(
                        div()
                            .id("close-profile")
                            .absolute()
                            .top_3()
                            .right_3()
                            .size(zoomed(32.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .bg(theme::hover())
                            .cursor_pointer()
                            .hover(|s| s.bg(theme::selected()))
                            .child(icons::icon(icons::CLOSE, 18., theme::interactive()))
                            .on_click(cx.listener(|this, _, window, cx| this.close_overlay(window, cx))),
                    ),
            )
    }

    /// The left column under the avatar: name, tag and badges, the
    /// actions, the bio and what the user knows of the person.
    fn profile_person(&self, user: Snowflake, guild: Option<Snowflake>, card: &Card, cx: &mut Context<Self>) -> Div {
        let text_color = card.theme.map_or(theme::text_strong(), |t| t.text);
        let me = self.people.is_me(user);
        let guild_name = guild.map(|id| self.spaces.iter().find(|s| s.id == id).map(|s| s.name.clone()).unwrap_or_default());
        let kind = self.people.relationship(user);
        let button_color = card.theme.map_or(theme::brand(), |t| t.button);
        let actions = (!me).then(|| {
            div()
                .flex()
                .gap_2()
                .child(
                    div()
                        .id("profile-message")
                        .flex_1()
                        .h(zoomed(36.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap_2()
                        .rounded(theme::radius::control())
                        .bg(rgb(button_color))
                        .cursor_pointer()
                        .hover(|s| s.opacity(0.9))
                        .text_size(zoomed(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(0xffffff))
                        .child(icons::icon(icons::CHAT_BUBBLE, 18., 0xffffff))
                        .child(t!("profile.message"))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.close_overlay(window, cx);
                            this.open_dm(user, window, cx);
                        })),
                )
                .when(kind.is_none(), |row| {
                    row.child(
                        widgets::button("profile-add-friend", t!("profile.add_friend"), ButtonKind::Secondary)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.net.post(Command::AcceptFriend { user });
                                cx.notify();
                            })),
                    )
                })
                .child({
                    let spot = widgets::popover::Tracked::default();
                    spot.track(
                        div()
                            .id("profile-more")
                            .size(zoomed(36.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(theme::radius::control())
                            .bg(theme::hover())
                            .cursor_pointer()
                            .child(crate::icons::icon(crate::icons::MORE, 18., theme::interactive())),
                    )
                    .on_click({
                        let spot = spot.clone();
                        cx.listener(move |this, _, _, cx| {
                            this.open_user_menu(user, guild, None, widgets::popover::Anchor::below_end(spot.bounds()), cx)
                        })
                    })
                })
        });
        let friends_since = (kind == Some(relationship_kind::FRIEND))
            .then(|| self.people.friends_since(user))
            .flatten();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .px_5()
            .pt(zoomed(AVATAR / 2. + RING + 12.))
            .pb_5()
            .text_color(rgb(text_color))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        widgets::name::styled_name(card.name.clone(), card.name_style.as_ref(), text_color, true, super::super::state::profile_theme::card_background(card.theme.as_ref()), cx)
                            .text_size(zoomed(24.))
                            .font_weight(FontWeight::BOLD),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_1()
                            .text_size(zoomed(14.))
                            .child(card.username.clone())
                            .children(card.pronouns.clone().map(|p| format!("\u{2022} {p}")))
                            .children(card.tag.as_ref().map(|tag| {
                                let badge = tag.badge_url(32).and_then(|url| self.avatar_image(&url));
                                widgets::name::tag_chip("modal-tag", tag.tag.as_deref().unwrap_or_default(), badge, 16.)
                            }))
                            .children(self.badges_pill("modal-badge", card)),
                    ),
            )
            .children(actions)
            .children(self.activities(user))
            .children(card.bio.clone().map(|bio| div().text_size(zoomed(14.)).child(bio)))
            .child(small_section(t!("profile.member_since"), member_since(user, card, guild_name)))
            .children(friends_since.map(|since| small_section(t!("profile.friends_since"), text(time::short_date(since)))))
            .children(roles(card))
            .children(connections(card))
            .when(!me, |column| column.child(small_section(t!("profile.note"), self.note_field())))
    }

    /// What the open tab shows. The Board leads when there is one.
    fn profile_tab_body(&self, user: Snowflake, guild: Option<Snowflake>, cx: &mut Context<Self>) -> Div {
        let profile = self.profiles.get(&(user, guild)).or_else(|| self.profiles.get(&(user, None)));
        match self.shown_profile_tab(user, guild) {
            ProfileTab::About => self.board(profile),
            ProfileTab::Servers => self.mutual_servers(user, guild, cx),
            ProfileTab::Friends => self.mutual_friends_list(user, guild, cx),
        }
    }

    /// The tab on screen: Mutual Friends stands in for a Board there is not.
    pub(super) fn shown_profile_tab(&self, user: Snowflake, guild: Option<Snowflake>) -> ProfileTab {
        let profile = self.profiles.get(&(user, guild)).or_else(|| self.profiles.get(&(user, None)));
        match self.profile_tab {
            ProfileTab::About if !self.has_board(profile) && !self.people.is_me(user) => ProfileTab::Friends,
            tab => tab,
        }
    }
}

/// A section of the left column under a 12px heading.
fn small_section(title: &'static str, body: impl IntoElement) -> Div {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_size(zoomed(12.)).font_weight(FontWeight::SEMIBOLD).child(title))
        .child(body)
}

/// Connected accounts, each a small box with the service and the name.
fn connections(card: &Card) -> Option<Div> {
    (!card.accounts.is_empty()).then(|| {
        small_section(
            t!("profile.connections"),
            div().flex().flex_wrap().gap_2().children(card.accounts.iter().map(|(kind, name)| {
                div()
                    .flex()
                    .flex_col()
                    .px_3()
                    .py_1()
                    .rounded(theme::radius::card())
                    .border_1()
                    .border_color(theme::border())
                    .child(div().text_xs().opacity(0.8).child(kind.clone()))
                    .child(div().text_size(zoomed(14.)).font_weight(FontWeight::SEMIBOLD).child(name.clone()))
            })),
        )
    })
}

/// The member's roles in the guild, as colored pills.
fn roles(card: &Card) -> Option<Div> {
    (!card.roles.is_empty()).then(|| {
        small_section(
            t!("profile.roles"),
            div().flex().flex_wrap().gap_1().children(card.roles.iter().map(|(name, color)| {
                // Roles without a color are drawn grey, like Discord does.
                let color = if *color == 0 { theme::interactive_muted() } else { *color };
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_2()
                    .py(zoomed(2.))
                    .rounded(theme::radius::tag())
                    .bg(theme::hover())
                    .text_size(zoomed(12.))
                    .child(div().size(zoomed(10.)).flex_none().rounded_full().bg(rgb(color)))
                    .child(name.clone())
            })),
        )
    })
}
