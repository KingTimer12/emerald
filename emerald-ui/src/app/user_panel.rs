//! The floating panel with the user's avatar, name and status.

use gpui::{Context, FontWeight, SharedString, div, prelude::*, rgb};

use super::super::{icons, theme};
use super::settings::Page;
use super::widgets::popover::Anchor;
use super::{Emerald, widgets};
use emerald_discord::model::Snowflake;
use crate::zoom::zoomed;

impl Emerald {
    /// Status shown on the user's own avatar: the picked one, else online
    /// while connected.
    pub(super) fn own_status(&self) -> Option<&'static str> {
        if !self.connection.connected || self.connection.fatal.is_some() {
            return None;
        }
        Some(self.settings.status.map_or("online", |s| s.as_str()))
    }

    pub(super) fn render_user_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let me = self.people.me();
        let name: SharedString = me.map_or("—".into(), |me| me.display_name().to_owned().into());
        let username: SharedString = me.map_or(SharedString::default(), |me| me.username.clone().into());
        let status = self.own_status();
        let custom = self.account.custom_status(super::account::now_ms()).map(|c| c.line());
        let (subtitle, subtitle_color) = match &self.connection.fatal {
            Some(fatal) => (fatal.clone(), theme::danger()),
            // Like the web client, the custom status stands in for the status.
            None if status.is_some() && custom.is_some() => (custom.unwrap_or_default().into(), theme::text_muted()),
            None if status.is_some() => (widgets::status_label(status).into(), theme::text_muted()),
            None => (self.connection.status.clone(), theme::text_muted()),
        };

        // The popover opens over the whole panel, its left edge lined up.
        let panel = self.user_panel.clone();
        panel
            .track(div())
            .absolute()
            .left_2()
            .bottom_2()
            .w(zoomed(super::sidebar::SERVER_LIST_WIDTH + super::sidebar::SIDEBAR_WIDTH - 16.))
            .h(zoomed(56.))
            .px_2()
            .flex()
            .items_center()
            .gap_1()
            .rounded(theme::radius::card())
            .border_1()
            .border_color(theme::border())
            .bg(rgb(theme::surface_high()))
            .child(
                // Avatar and name open the profile popover, as in Discord 2026.
                div()
                    .id("me")
                    .group("me")
                    .flex()
                    .flex_1()
                    .min_w(zoomed(0.))
                    .items_center()
                    .gap_2()
                    .p_1()
                    .rounded(theme::radius::control())
                    .cursor_pointer()
                    .hover(|s| s.bg(theme::hover()))
                    .child(widgets::avatar(
                        &name,
                        me.map_or(Snowflake(0), |me| me.id),
                        me.and_then(|me| self.profile_avatar(me.id)),
                        32.,
                        Some((status.unwrap_or("offline"), theme::surface_high())),
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(zoomed(0.))
                            .child(
                                div()
                                    .truncate()
                                    .text_size(zoomed(14.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(theme::text_default()))
                                    .child(name.clone()),
                            )
                            // The username takes the status line's place while
                            // the block is hovered, as on Discord.
                            .child(
                                div()
                                    .relative()
                                    .child(
                                        div()
                                            .truncate()
                                            .text_xs()
                                            .text_color(rgb(subtitle_color))
                                            .group_hover("me", |s| s.opacity(0.))
                                            .child(subtitle),
                                    )
                                    .child(
                                        div()
                                            .absolute()
                                            .inset_0()
                                            .truncate()
                                            .text_xs()
                                            .text_color(rgb(theme::text_muted()))
                                            .opacity(0.)
                                            .group_hover("me", |s| s.opacity(1.))
                                            .child(username),
                                    ),
                            ),
                    )
                    .on_click(cx.listener({
                        let panel = panel.clone();
                        move |this, _, _, cx| this.toggle_own_popover(Anchor::above(panel.bounds()), cx)
                    })),
            )
            .child(self.render_voice_buttons(cx))
            .child(
                widgets::svg_button("open-settings", icons::SETTINGS)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_settings(Page::Account, window, cx)
                    })),
            )
    }
}
