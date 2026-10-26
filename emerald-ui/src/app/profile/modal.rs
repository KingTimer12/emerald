//! The full profile, opened over the window from a name or an avatar.

use super::*;

impl Emerald {
    pub(in super::super) fn render_profile_modal(
        &self,
        user: Snowflake,
        guild: Option<Snowflake>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let card = self.card(user, guild);
        let guild_name = guild.map(|id| self.spaces.iter().find(|s| s.id == id).map(|s| s.name.clone()).unwrap_or_default());
        const BANNER: f32 = 210.;
        const AVATAR: f32 = 120.;
        const RING: f32 = 8.;

        let accounts = (!card.accounts.is_empty()).then(|| {
            section(
                t!("profile.connections"),
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

        let roles = (!card.roles.is_empty()).then(|| {
            section(
                t!("profile.roles"),
                div().flex().flex_wrap().gap_1().children(card.roles.iter().map(|(name, color)| {
                    // Roles without a color are drawn grey, like Discord does.
                    let color = if *color == 0 { theme::INTERACTIVE_MUTED } else { *color };
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .px_2()
                        .py(px(2.))
                        .rounded_md()
                        .bg(theme::hover())
                        .text_size(px(12.))
                        .text_color(rgb(theme::TEXT_DEFAULT))
                        .child(div().size(px(10.)).flex_none().rounded_full().bg(rgb(color)))
                        .child(name.clone())
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
                    .child(self.banner(&card, BANNER))
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
                            .map(|body| {
                                if self.people.is_me(user) {
                                    return body
                                        .child(div().w_full().h(px(1.)).bg(theme::border()))
                                        .children(card.bio.clone().map(|bio| section(t!("profile.about"), text(bio))))
                                        .child(section(t!("profile.member_since"), member_since(user, &card, guild_name)))
                                        .children(roles)
                                        .children(accounts);
                                }
                                let body = body.child(self.profile_tabs(user, guild, cx));
                                match self.profile_tab {
                                    ProfileTab::About => body
                                        .children(card.bio.clone().map(|bio| section(t!("profile.about"), text(bio))))
                                        .child(section(t!("profile.member_since"), member_since(user, &card, guild_name)))
                                        .children(roles)
                                        .children(accounts)
                                        .child(self.note_section()),
                                    ProfileTab::Servers => body.child(self.mutual_servers(user, guild, cx)),
                                    ProfileTab::Friends => body.child(self.mutual_friends_list(user, cx)),
                                }
                            }),
                    ),
            )
    }
}
