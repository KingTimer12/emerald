//! The full profile, opened over the window from a name or an avatar.

use super::*;
use crate::zoom::zoomed;

/// Sized to fit a laptop screen; whatever is taller scrolls inside.
const WIDTH: f32 = 560.;
const BANNER: f32 = 150.;
const AVATAR: f32 = 96.;
const RING: f32 = 6.;

impl Emerald {
    pub(in super::super) fn render_profile_modal(
        &self,
        user: Snowflake,
        guild: Option<Snowflake>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let card = self.card(user, guild);
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
                    .w(zoomed(WIDTH))
                    .max_w(gpui::relative(0.9))
                    .max_h(gpui::relative(0.9))
                    .flex()
                    .flex_col()
                    .rounded_lg()
                    .overflow_hidden()
                    .border_1()
                    .border_color(theme::border())
                    .bg(rgb(theme::base_lowest()))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        // Banner, avatar and body scroll together; the close
                        // button stays put above them.
                        div()
                            .id("profile-scroll")
                            .w_full()
                            .flex_shrink()
                            .min_h(zoomed(0.))
                            .overflow_y_scroll()
                            .child(
                                div()
                                    .relative()
                                    .w_full()
                                    .flex()
                                    .flex_col()
                                    .child(self.banner(&card, BANNER))
                                    .child(
                                        div()
                                            .absolute()
                                            .top(zoomed(BANNER - AVATAR / 2. - RING))
                                            .left_5()
                                            .child(self.avatar_ring(user, &card, AVATAR, RING)),
                                    )
                                    .child(self.profile_body(user, guild, &card, cx)),
                            ),
                    )
                    // Over the whole card, still while its content scrolls.
                    .children(self.effect(&card, WIDTH))
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
                    ),
            )
    }

    /// Name, tabs and what the open tab shows.
    fn profile_body(&self, user: Snowflake, guild: Option<Snowflake>, card: &Card, cx: &mut Context<Self>) -> Div {
        let guild_name = guild.map(|id| self.spaces.iter().find(|s| s.id == id).map(|s| s.name.clone()).unwrap_or_default());
        let body = div()
            .w_full()
            .flex()
            .flex_col()
            .gap_3()
            .px_5()
            .pt(zoomed(AVATAR / 2. + RING + 12.))
            .pb_5()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_size(zoomed(20.))
                            .line_height(zoomed(26.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(theme::text_strong()))
                            .child(card.name.clone()),
                    )
                    .child(self.name_line(card, 14., 22.))
                    .children(card.pronouns.clone().map(|p| {
                        div()
                            .text_size(zoomed(14.))
                            .text_color(rgb(theme::text_muted()))
                            .child(p)
                    })),
            );
        let about = |body: Div| {
            body.children(card.bio.clone().map(|bio| section(t!("profile.about"), text(bio))))
                .child(section(t!("profile.member_since"), member_since(user, card, guild_name.clone())))
                .children(roles(card))
                .children(connections(card))
        };
        if self.people.is_me(user) {
            return about(body.child(div().w_full().h(zoomed(1.)).bg(theme::border())));
        }
        let body = body.child(self.profile_tabs(user, guild, cx));
        match self.profile_tab {
            ProfileTab::About => about(body).child(self.note_section()),
            ProfileTab::Servers => body.child(self.mutual_servers(user, guild, cx)),
            ProfileTab::Friends => body.child(self.mutual_friends_list(user, cx)),
        }
    }
}

/// Connected accounts, each a small box with the service and the name.
fn connections(card: &Card) -> Option<Div> {
    (!card.accounts.is_empty()).then(|| {
        section(
            t!("profile.connections"),
            div().flex().flex_wrap().gap_2().children(card.accounts.iter().map(|(kind, name)| {
                div()
                    .flex()
                    .flex_col()
                    .px_3()
                    .py_1()
                    .rounded_lg()
                    .border_1()
                    .border_color(theme::border())
                    .child(div().text_xs().text_color(rgb(theme::text_muted())).child(kind.clone()))
                    .child(
                        div()
                            .text_size(zoomed(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::text_strong()))
                            .child(name.clone()),
                    )
            })),
        )
    })
}

/// The member's roles in the guild, as colored pills.
fn roles(card: &Card) -> Option<Div> {
    (!card.roles.is_empty()).then(|| {
        section(
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
                    .rounded_md()
                    .bg(theme::hover())
                    .text_size(zoomed(12.))
                    .text_color(rgb(theme::text_default()))
                    .child(div().size(zoomed(10.)).flex_none().rounded_full().bg(rgb(color)))
                    .child(name.clone())
            })),
        )
    })
}
