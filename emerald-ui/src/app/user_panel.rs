//! The floating panel with the user's avatar, name and status.

use gpui::{Context, FontWeight, SharedString, div, prelude::*, px, rgb};

use super::super::{icons, theme};
use super::settings::Page;
use super::{Emerald, widgets};
use emerald_discord::model::Snowflake;

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
        let status = self.own_status();
        let (subtitle, subtitle_color) = match &self.connection.fatal {
            Some(fatal) => (fatal.clone(), theme::DANGER),
            None if status.is_some() => (widgets::status_label(status).into(), theme::TEXT_MUTED),
            None => (self.connection.status.clone(), theme::TEXT_MUTED),
        };

        div()
            .absolute()
            .left_2()
            .bottom_2()
            .w(px(72. + 240. - 16.))
            .h(px(56.))
            .px_2()
            .flex()
            .items_center()
            .gap_1()
            .rounded_lg()
            .border_1()
            .border_color(theme::border())
            .bg(rgb(theme::BASE_LOW))
            .child(
                // Avatar and name open the status picker, like Discord's popout.
                div()
                    .id("me")
                    .flex()
                    .flex_1()
                    .min_w(px(0.))
                    .items_center()
                    .gap_2()
                    .p_1()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme::hover()))
                    .child(widgets::avatar(
                        &name,
                        me.map_or(Snowflake(0), |me| me.id),
                        me.and_then(|me| self.profile_avatar(me.id)),
                        32.,
                        Some((status.unwrap_or("offline"), theme::BASE_LOW)),
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(px(0.))
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(theme::TEXT_DEFAULT))
                                    .child(name.clone()),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_xs()
                                    .text_color(rgb(subtitle_color))
                                    .child(subtitle),
                            ),
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_settings(Page::Status, window, cx)
                    })),
            )
            .child(
                widgets::svg_button("open-settings", icons::SETTINGS)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_settings(Page::Account, window, cx)
                    })),
            )
    }
}
