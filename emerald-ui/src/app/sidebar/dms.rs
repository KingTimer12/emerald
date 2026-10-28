//! Rows only the DM sidebar has: "Message Requests" and the "+" that
//! starts a conversation.

use super::*;
use crate::zoom::zoomed;

impl Emerald {
    /// "Message Requests" under "Friends", with how many wait.
    pub(super) fn render_requests_row(&self, row: gpui::Stateful<gpui::Div>, cx: &mut Context<Self>) -> AnyElement {
        let selected = self.requests_open;
        let count = self.spaces.dms().requests(false).count();
        let color = if selected { theme::text_strong() } else { theme::channels_default() };
        row.gap_3()
            .rounded_lg()
            .cursor_pointer()
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(color))
            .when(selected, |r| r.bg(theme::selected()))
            .when(!selected, |r| r.hover(|s| s.bg(theme::hover()).text_color(rgb(theme::text_strong()))))
            .child(div().flex_none().w(zoomed(32.)).flex().justify_center().child(icon(icons::INBOX, 22., color)))
            .child(div().flex_1().child(t!("requests.title")))
            .when(count > 0, |r| r.child(widgets::badge(count)))
            .on_click(cx.listener(|this, _, _, cx| this.open_requests(cx)))
            .into_any_element()
    }

    /// The "+" beside "Direct Messages": friends for a new DM or group.
    pub(super) fn new_dm_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("new-dm")
            .ml_auto()
            .rounded_sm()
            .cursor_pointer()
            .tooltip(widgets::tooltip(t!("group.create_dm_tooltip")))
            .child(icon(icons::PLUS, 16., theme::channels_default()))
            .on_click(cx.listener(|this, _, window, cx| this.open_group_picker(None, window, cx)))
    }

    /// A group DM's picture, where a 1:1 DM shows the other person.
    pub(super) fn group_picture(&self, url: &str) -> AnyElement {
        let tile = div().size(zoomed(32.)).flex_none().rounded_full().overflow_hidden().bg(theme::hover());
        match self.avatar_image(url) {
            Some(image) => tile.child(img(image).size_full()).into_any_element(),
            None => tile.into_any_element(),
        }
    }
}
