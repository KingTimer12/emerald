//! A server's header atop its channels: the banner when it has one, its
//! name, the invite button, and the boost goal bar when it turned it on.

use gpui::{AnyElement, Context, FontWeight, div, img, prelude::*, rgb};

use super::super::state::spaces::BoostGoal;
use super::super::{Emerald, widgets};
use crate::icons::{self, icon};
use crate::theme;
use crate::zoom::zoomed;

/// Header height without a banner, and with one, as on discord.com.
const PLAIN: f32 = 48.;
const WITH_BANNER: f32 = 136.;
/// The boost bar's pink, `--guild-boosting-pink`.
const BOOST_PINK: u32 = 0xff73fa;

impl Emerald {
    pub(super) fn render_guild_header(&self, cx: &mut Context<Self>) -> AnyElement {
        let space = self.spaces.current();
        let header = space.header().cloned().unwrap_or_default();
        let banner = header.banner.as_ref().and_then(|url| self.avatar_image(url));
        let on_banner = banner.is_some();
        let invite = self.invite_channel(space.id).map(|channel| {
            widgets::svg_button("guild-invite", icons::USER_ADD)
                .tooltip(widgets::tooltip(t!("invite.invite_people")))
                .on_click(cx.listener(move |this, _, _, cx| this.open_invite(channel, "Guild Header Invite Button", cx)))
        });
        let text = if on_banner { theme::on_color() } else { theme::text_strong() };
        let title = div()
            .h(zoomed(PLAIN))
            .px_4()
            .flex()
            .items_center()
            .gap_1()
            .text_color(rgb(text))
            .font_weight(FontWeight::SEMIBOLD)
            .child(div().flex_1().min_w(zoomed(0.)).truncate().child(space.name.clone()))
            .child(icon(icons::CHEVRON, 18., if on_banner { text } else { theme::interactive() }))
            .children(invite);

        let top = match banner {
            Some(banner) => div()
                .relative()
                .h(zoomed(WITH_BANNER))
                .flex_none()
                .overflow_hidden()
                .child(img(banner).absolute().inset_0().size_full().object_fit(gpui::ObjectFit::Cover))
                // A shade behind the name so white text reads on any banner.
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .bg(theme::with_alpha(0x000000, 0.35))
                        .child(title),
                ),
            None => div().flex_none().border_b_1().border_color(theme::border()).child(title),
        };
        div()
            .flex_none()
            .flex()
            .flex_col()
            .child(top)
            .children(header.boost.map(boost_bar))
            .into_any_element()
    }
}

/// "Boost Goal" with "14/33 Boosts" and the pink fill. Display only: the
/// boosting page it opens on discord.com sells Nitro, which Emerald leaves out.
fn boost_bar(goal: BoostGoal) -> impl IntoElement {
    let full = goal.count >= goal.goal;
    let count = if full {
        t!("guild.boosts", count = goal.count)
    } else {
        t!("guild.boosts_of", count = goal.count, goal = goal.goal)
    };
    div()
        .mx_2()
        .mt_2()
        .p_2()
        .flex()
        .flex_col()
        .gap_1p5()
        .rounded(theme::radius::card())
        .border_1()
        .border_color(theme::border_normal())
        .text_size(zoomed(12.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(
            div()
                .flex()
                .justify_between()
                .child(div().text_color(rgb(theme::text_strong())).child(t!("guild.boost_goal")))
                .child(div().text_color(rgb(theme::text_muted())).child(count)),
        )
        .child(
            div()
                .h(zoomed(6.))
                .rounded_full()
                .bg(rgb(theme::interactive_muted()))
                .child(div().h_full().w(gpui::relative(goal.fraction())).rounded_full().bg(rgb(BOOST_PINK))),
        )
}
