//! The window's title bar, drawn by Emerald as Discord 2026 draws its own:
//! back and forward on the left, the page on screen in the middle, Inbox and
//! Help on the right, then (on Windows) the window's buttons.

use gpui::{AnyElement, Context, FontWeight, SharedString, Window, WindowControlArea, div, img, prelude::*, rgb};

use super::panels::PanelKind;
use super::state::Space;
use super::{Emerald, widgets};
use crate::icons;
use crate::theme;
use crate::zoom::zoomed;
use emerald_discord::model::Snowflake;
use emerald_discord::urls;

mod controls;

/// Discord 2026's title bar height.
const HEIGHT: f32 = 32.;
/// Room left for macOS's traffic lights, drawn by the system over the bar.
const TRAFFIC_LIGHTS: f32 = 72.;
/// The top edge left out of the drag area while the window is not maximized,
/// so Windows can still resize it from there.
const RESIZE_EDGE: f32 = 4.;

/// What the chat column shows, for naming the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Screen {
    Friends,
    Requests,
    Channel,
}

/// The picture beside the page's name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TitleIcon {
    Friends,
    Requests,
    /// A server's icon, or its initials.
    Guild(Snowflake),
    /// The other person's avatar in a 1:1 DM.
    User(Snowflake),
    /// A group DM's picture.
    Group(Snowflake),
    /// Home with nothing open.
    None,
}

/// The page's name as Discord's title bar gives it: the server for any of
/// its channels, the person or group for a DM, else the Home page shown.
pub(super) fn page_title(space: &Space, screen: Screen, channel: Option<Snowflake>) -> (TitleIcon, SharedString) {
    if !space.is_dms() {
        return (TitleIcon::Guild(space.id), space.name.clone());
    }
    match screen {
        Screen::Friends => (TitleIcon::Friends, t!("friends.title").into()),
        Screen::Requests => (TitleIcon::Requests, t!("requests.title").into()),
        Screen::Channel => match channel.and_then(|id| space.row(id)) {
            Some(row) => match row.user {
                Some(user) => (TitleIcon::User(user), row.name.clone()),
                None => (TitleIcon::Group(row.id), row.name.clone()),
            },
            None => (TitleIcon::None, SharedString::default()),
        },
    }
}

impl Emerald {
    /// The bar across the top of the window. Its empty parts drag the window;
    /// a double-click there maximizes it, as the system's own bar would.
    pub(super) fn render_title_bar(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let screen = if self.friends_open {
            Screen::Friends
        } else if self.requests_open {
            Screen::Requests
        } else {
            Screen::Channel
        };
        let (title_icon, title) = page_title(self.spaces.current(), screen, self.selected_channel);
        let back = self.places.can_go_back();
        let forward = self.places.can_go_forward();

        let left = div()
            .flex()
            .items_center()
            .gap_1()
            .pl(zoomed(if cfg!(target_os = "macos") { TRAFFIC_LIGHTS } else { 8. }))
            .child(
                nav_button("title-back", icons::ARROW_LEFT, t!("title_bar.back"), back)
                    .when(back, |b| b.on_click(cx.listener(|this, _, window, cx| this.go_back(window, cx)))),
            )
            .child(
                nav_button("title-forward", icons::ARROW_RIGHT, t!("title_bar.forward"), forward)
                    .when(forward, |b| b.on_click(cx.listener(|this, _, window, cx| this.go_forward(window, cx)))),
            );

        // Centered on the window, not between the two sides.
        let center = div().absolute().inset_0().flex().items_center().justify_center().child(
            div()
                .max_w(gpui::relative(0.5))
                .flex()
                .items_center()
                .gap_2()
                .children(self.title_picture(title_icon, &title))
                .child(
                    div()
                        .min_w(zoomed(0.))
                        .truncate()
                        .text_size(zoomed(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(theme::text_strong()))
                        .child(title),
                ),
        );

        let inbox_open = self.panel_is(PanelKind::Inbox);
        let right = div()
            .flex()
            .items_center()
            .h_full()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_2()
                    .child(
                        bar_button("title-inbox", icons::INBOX, t!("inbox.title"), inbox_open)
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_inbox(cx))),
                    )
                    .child(
                        bar_button("title-help", icons::HELP, t!("title_bar.help"), false)
                            .on_click(|_, _, cx| cx.open_url(urls::SUPPORT.as_str())),
                    ),
            )
            .when(cfg!(target_os = "windows"), |right| right.child(controls::window_controls(window)));

        // Drawn first, so the buttons above it (which occlude) take their own
        // clicks; the rest of the bar, title included, moves the window.
        let drag = div()
            .absolute()
            .top(if window.is_maximized() { zoomed(0.) } else { gpui::px(RESIZE_EDGE) })
            .left_0()
            .right_0()
            .bottom_0()
            .window_control_area(WindowControlArea::Drag);

        div()
            .id("title-bar")
            .relative()
            .flex_none()
            .h(zoomed(HEIGHT))
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .bg(rgb(theme::base_lowest()))
            // Windows hands a press on the bar to the window before moving
            // or resizing it, and gives up if anything handles the press.
            // The root takes focus on every press it gets, so the bar keeps
            // presses from reaching it.
            .occlude()
            .child(drag)
            // Windows answers double-clicks on the drag area itself.
            .when(cfg!(target_os = "macos"), |bar| {
                bar.on_click(|event, window, _| {
                    if event.click_count() == 2 {
                        window.titlebar_double_click();
                    }
                })
            })
            .child(center)
            .child(left)
            .child(right)
            .into_any_element()
    }

    /// The 16px picture beside the page's name.
    fn title_picture(&self, title_icon: TitleIcon, title: &str) -> Option<AnyElement> {
        const SIZE: f32 = 16.;
        let color = theme::interactive();
        Some(match title_icon {
            TitleIcon::Friends => icons::icon(icons::FRIENDS, SIZE, color).into_any_element(),
            TitleIcon::Requests => icons::icon(icons::INBOX, SIZE, color).into_any_element(),
            TitleIcon::Guild(_) => {
                let space = self.spaces.current();
                let tile = div()
                    .size(zoomed(SIZE))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .overflow_hidden()
                    .rounded(theme::radius::server_icon_at(SIZE))
                    .bg(rgb(theme::base_low()))
                    .text_size(zoomed(7.))
                    .text_color(rgb(theme::text_strong()));
                match space.icon_url.as_ref().and_then(|url| self.avatar_image(url)) {
                    Some(image) => tile.child(img(image).size_full()).into_any_element(),
                    None => tile.child(space.initials.clone()).into_any_element(),
                }
            }
            TitleIcon::User(user) => {
                widgets::avatar(title, user, self.user_avatar(user), SIZE, None).into_any_element()
            }
            TitleIcon::Group(channel) => {
                let picture = self.spaces.current().row(channel).and_then(|row| row.picture.clone());
                match picture.and_then(|url| self.avatar_image(&url)) {
                    Some(image) => div()
                        .size(zoomed(SIZE))
                        .flex_none()
                        .rounded_full()
                        .overflow_hidden()
                        .child(img(image).size_full())
                        .into_any_element(),
                    None => icons::icon(icons::FRIENDS, SIZE, color).into_any_element(),
                }
            }
            TitleIcon::None => return None,
        })
    }
}

/// Back or forward: dimmed, and deaf to clicks, when there is nowhere to go.
fn nav_button(id: &'static str, glyph: &'static str, tip: &'static str, enabled: bool) -> gpui::Stateful<gpui::Div> {
    bar_button(id, glyph, tip, false).when(!enabled, |b| b.opacity(0.4).cursor_default())
}

/// A 24px icon on the bar. It occludes the drag area under it, so a click
/// reaches the button instead of moving the window.
fn bar_button(id: &'static str, glyph: &'static str, tip: &'static str, active: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .size(zoomed(24.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme::radius::control())
        .cursor_pointer()
        .hover(|s| s.bg(theme::hover()))
        .tooltip(widgets::tooltip(tip))
        .occlude()
        .child(icons::icon(glyph, 18., if active { theme::text_strong() } else { theme::interactive() }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use emerald_discord::model::Guild;

    fn guild_space() -> Space {
        let guild = serde_json::from_value::<Guild>(serde_json::json!({ "id": "100", "name": "illusia" })).unwrap();
        Space::guild(guild, Snowflake(1), Vec::new())
    }

    #[test]
    fn page_title_names_friends_on_home() {
        let home = Space::dms(Vec::new());
        assert_eq!(page_title(&home, Screen::Friends, None), (TitleIcon::Friends, t!("friends.title").into()));
    }

    #[test]
    fn page_title_names_message_requests() {
        let home = Space::dms(Vec::new());
        assert_eq!(page_title(&home, Screen::Requests, None), (TitleIcon::Requests, t!("requests.title").into()));
    }

    #[test]
    fn page_title_names_the_server_in_any_of_its_channels() {
        let space = guild_space();
        let title = page_title(&space, Screen::Channel, Some(Snowflake(5)));
        assert_eq!(title, (TitleIcon::Guild(Snowflake(100)), SharedString::from("illusia")));
    }
}
