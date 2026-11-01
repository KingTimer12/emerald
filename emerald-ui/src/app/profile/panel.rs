//! The profile beside a 1:1 DM, in Discord 2026's form: themed card,
//! banner with the "More" button, name, tag and badges, what both share,
//! the bio and "Member Since".

use super::*;

impl Emerald {
    pub(in super::super) fn render_profile_panel(&self, user: Snowflake, cx: &mut Context<Self>) -> impl IntoElement {
        let card = self.card(user, None);
        // `self.card` names people by their own name; the panel sits by
        // the DM, where a friend goes by the nickname given.
        let card = Card {
            name: self.people.nickname(user).map_or(card.name.clone(), |n| n.to_owned().into()),
            ..card
        };
        const BANNER: f32 = 120.;
        const AVATAR: f32 = 80.;
        const RING: f32 = 6.;
        let text_color = card.theme.map_or(theme::text_strong(), |t| t.text);
        let more = widgets::popover::Tracked::default();
        let buttons = div().absolute().top_2().right_2().flex().gap_1().child(
            more.track(
                div()
                    .id("panel-more")
                    .size(zoomed(32.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(gpui::rgba(0x0000_0099))
                    .cursor_pointer()
                    .child(crate::icons::icon(crate::icons::MORE, 18., 0xffffff)),
            )
            .on_click({
                let more = more.clone();
                cx.listener(move |this, _, _, cx| {
                    this.open_user_menu(user, None, None, widgets::popover::Anchor::below_end(more.bounds()), cx)
                })
            }),
        );
        let username_line = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_1()
            .text_size(zoomed(14.))
            .child(card.username.clone())
            .children(card.pronouns.clone().map(|p| format!("\u{2022} {p}")))
            .children(card.tag.as_ref().map(|tag| {
                let badge = tag.badge_url(28).and_then(|url| self.avatar_image(&url));
                widgets::name::tag_chip("panel-tag", tag.tag.as_deref().unwrap_or_default(), badge, 14.)
            }))
            .children(self.badges_pill("panel-badge", &card));

        div().w(zoomed(PANEL_WIDTH)).flex_none().h_full().p_2().child(
            self.themed(
                div()
                    .relative()
                    .size_full()
                    .flex()
                    .flex_col()
                    .rounded(theme::radius::card())
                    .overflow_hidden()
                    .border_1()
                    .border_color(theme::border()),
                card.theme.as_ref(),
            )
            .child(div().relative().child(self.banner(&card, BANNER)).child(buttons))
            .child(
                // Avatar straddling the banner's bottom edge.
                div()
                    .absolute()
                    .top(zoomed(BANNER - AVATAR / 2. - RING))
                    .left_4()
                    .child(self.avatar_ring(user, &card, AVATAR, RING)),
            )
            .child(
                div()
                    .id("profile-body")
                    .relative()
                    .w_full()
                    .flex_1()
                    .min_h(zoomed(0.))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .px_4()
                    .pt(zoomed(AVATAR / 2. + RING + 12.))
                    .pb_4()
                    .text_color(rgb(text_color))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                widgets::name::styled_name(card.name.clone(), card.name_style.as_ref(), text_color, true, super::super::state::profile_theme::card_background(card.theme.as_ref()), cx)
                                    .text_size(zoomed(20.))
                                    .line_height(zoomed(24.))
                                    .font_weight(FontWeight::BOLD),
                            )
                            .child(username_line),
                    )
                    .children(self.mutual_row(&card, user, None, cx))
                    .children(self.activities(user))
                    .children(card.bio.clone().map(|bio| div().text_size(zoomed(14.)).child(bio)))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_size(zoomed(12.)).font_weight(FontWeight::SEMIBOLD).child(t!("profile.member_since")))
                            .child(text(time::short_date(user.unix_secs()))),
                    ),
            )
            .child(
                div().relative().w_full().flex_none().p_3().child(
                    widgets::button("full-profile", t!("profile.view_full"), ButtonKind::Secondary)
                        .w_full()
                        .on_click(cx.listener(move |this, _, window, cx| this.open_profile(user, None, window, cx)))
                        .on_mouse_down(MouseButton::Right, self.on_user_right_click(user, None, None, cx)),
                ),
            )
            .children(self.effect(&card, PANEL_WIDTH)),
        )
    }
}
