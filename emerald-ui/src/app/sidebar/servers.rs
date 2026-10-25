//! The server list: the DMs, then servers and folders in the user's order,
//! each with its unread dot and mention count.

use super::*;

impl Emerald {
    pub(in super::super) fn render_servers(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut items = Vec::with_capacity(self.spaces.len() + 2);
        items.push(self.render_space(0, self.spaces.dms(), cx).into_any_element());
        // Divider between direct messages and servers.
        items.push(
            div()
                .flex_none()
                .w(px(32.))
                .h(px(1.))
                .bg(theme::selected())
                .into_any_element(),
        );
        for item in self.server_list.items(&self.spaces.guild_ids()) {
            match item {
                Item::Guild(guild) => items.extend(self.render_guild(guild, cx)),
                Item::Folder(folder) => items.push(self.render_folder(folder, cx)),
            }
        }

        div()
            .id("servers")
            .flex()
            .flex_col()
            .items_center()
            .gap_2()
            .pt_1()
            .pb(px(PANEL_CLEARANCE))
            .w(px(72.))
            .h_full()
            .flex_none()
            .overflow_y_scroll()
            .children(items)
            .child(
                div()
                    .id("add-server")
                    .flex_none()
                    .size(px(40.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(12.))
                    .bg(rgb(theme::BASE_LOW))
                    .text_color(rgb(theme::ONLINE))
                    .text_size(px(24.))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::ONLINE)).text_color(rgb(theme::TEXT_STRONG)))
                    .child("+")
                    .on_click(cx.listener(|this, _, window, cx| this.open_add_server(window, cx))),
            )
    }

    fn render_guild(&self, guild: Snowflake, cx: &mut Context<Self>) -> Option<AnyElement> {
        let ix = self.spaces.index(guild)?;
        let space = self.spaces.iter().nth(ix)?;
        Some(self.render_space(ix, space, cx).into_any_element())
    }

    /// Whether `space` shows the unread dot: an unread channel outside
    /// what the user muted.
    fn space_unread(&self, space: &Space) -> bool {
        let now = crate::time::now_secs();
        let guild = (!space.is_dms()).then_some(space.id);
        if guild.is_some_and(|g| self.notify.guild_muted(g, now)) {
            return false;
        }
        space
            .channels()
            .iter()
            .filter(|c| self.reads.is_unread(c.id))
            .any(|c| !self.notify.muted(&self.place_of(guild, c.id), now))
    }

    /// A folder: closed, its first servers in a little grid; open, its
    /// icon above its servers on a shared background.
    fn render_folder(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(folder) = self.server_list.folder(ix) else { return div().into_any_element() };
        let open = self.server_list.is_open(ix);
        let color = folder.color.filter(|c| *c != 0).unwrap_or(theme::BRAND);
        let spaces: Vec<(usize, &Space)> = folder
            .guilds
            .iter()
            .filter_map(|g| self.spaces.index(*g).map(|ix| (ix, self.spaces.iter().nth(ix).expect("indexed"))))
            .collect();
        let mentions: u32 = spaces.iter().map(|(_, s)| self.reads.total_mentions(s.channels().iter())).sum();
        let unread = spaces.iter().any(|(_, s)| self.space_unread(s));
        let name: SharedString = folder
            .name
            .clone()
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| spaces.iter().map(|(_, s)| s.name.to_string()).collect::<Vec<_>>().join(", "))
            .into();
        let face = if open {
            div().child(icons::icon(icons::FOLDER, 24., color)).into_any_element()
        } else {
            // Up to four servers, two by two.
            div()
                .size(px(32.))
                .flex()
                .flex_wrap()
                .gap(px(2.))
                .children(spaces.iter().take(4).map(|(_, space)| {
                    let image = space.icon_url.as_ref().and_then(|url| self.avatar_image(url));
                    div()
                        .size(px(15.))
                        .rounded(px(5.))
                        .overflow_hidden()
                        .bg(rgb(theme::BASE_LOW))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(7.))
                        .text_color(rgb(theme::TEXT_STRONG))
                        .map(|tile| match image {
                            Some(image) => tile.child(img(image).size_full()),
                            None => tile.child(space.initials.clone()),
                        })
                }))
                .into_any_element()
        };
        let group: SharedString = format!("folder-{ix}").into();
        let button = div()
            .group(group.clone())
            .relative()
            .flex_none()
            .w_full()
            .h(px(40.))
            .flex()
            .justify_center()
            .when(unread && !open, |row| {
                row.child(
                    div()
                        .absolute()
                        .left_0()
                        .top(px(16.))
                        .w(px(4.))
                        .h(px(8.))
                        .rounded_r(px(4.))
                        .bg(rgb(theme::TEXT_STRONG)),
                )
            })
            .child(
                div()
                    .id(("folder", ix))
                    .size(px(40.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(12.))
                    .bg(if open { theme::hover() } else { theme::with_alpha(color, 0.4) })
                    .cursor_pointer()
                    .tooltip(widgets::tooltip(name))
                    .child(face)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.server_list.toggle(ix);
                        cx.notify();
                    })),
            )
            .when(mentions > 0 && !open, |row| {
                row.child(
                    div()
                        .absolute()
                        .bottom(px(-4.))
                        .right(px(10.))
                        .rounded_full()
                        .border_4()
                        .border_color(rgb(theme::BASE_LOWEST))
                        .child(widgets::badge(mentions as usize)),
                )
            });
        if !open {
            return button.into_any_element();
        }
        div()
            .flex_none()
            .w_full()
            .flex()
            .flex_col()
            .items_center()
            .child(
                div()
                    .w(px(48.))
                    .py_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_2()
                    .rounded(px(16.))
                    .bg(theme::with_alpha(color, 0.15))
                    .child(button)
                    .children(spaces.iter().map(|(ix, space)| self.render_space(*ix, space, cx).into_any_element())),
            )
            .into_any_element()
    }

    fn render_space(&self, ix: usize, space: &Space, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = ix == self.spaces.selected();
        let unread = !selected && self.space_unread(space);
        let guild = (!space.is_dms()).then_some(space.id);
        let mentions = self.reads.total_mentions(space.channels().iter());
        let group: SharedString = format!("space-{ix}").into();
        // White pill on the left edge: full height when selected, a dot when
        // unread, half height on hover.
        let (pill_height, pill_top) = match (selected, unread) {
            (true, _) => (40., 0.),
            (false, true) => (8., 16.),
            (false, false) => (0., 20.),
        };
        let pill = div()
            .absolute()
            .left_0()
            .top(px(pill_top))
            .w(px(4.))
            .h(px(pill_height))
            .rounded_r(px(4.))
            .bg(rgb(theme::TEXT_STRONG))
            .when(!selected, |pill| {
                pill.group_hover(group.clone(), |s| s.top(px(10.)).h(px(20.)))
            });
        let initials_size = if space.initials.chars().count() > 2 { 12. } else { 14. };
        let image = space.icon_url.as_ref().and_then(|url| self.avatar_image(url));

        div()
            .group(group.clone())
            .relative()
            .flex_none()
            .w_full()
            .h(px(40.))
            .flex()
            .justify_center()
            .child(pill)
            .child(
                div()
                    .id(("space", ix))
                    .size(px(40.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(12.))
                    .overflow_hidden()
                    .bg(rgb(if selected { theme::BRAND } else { theme::BASE_LOW }))
                    .text_color(rgb(if selected { theme::TEXT_STRONG } else { theme::INTERACTIVE }))
                    .text_size(px(initials_size))
                    .font_weight(FontWeight::MEDIUM)
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::BRAND)).text_color(rgb(theme::TEXT_STRONG)))
                    .tooltip(widgets::tooltip(space.name.clone()))
                    .map(|icon| match image {
                        Some(image) => icon.child(img(image).size_full()),
                        // Svg colors are not inherited, so hover is restated.
                        None if space.is_dms() => icon.child(
                            icons::icon(
                                icons::EMERALD,
                                24.,
                                if selected { theme::TEXT_STRONG } else { theme::INTERACTIVE },
                            )
                            .group_hover(group.clone(), |s| s.text_color(rgb(theme::TEXT_STRONG))),
                        ),
                        None => icon.child(space.initials.clone()),
                    })
                    .on_click(cx.listener(move |this, _, window, cx| this.select_space(ix, window, cx)))
                    .when_some(guild, |icon, guild| {
                        icon.on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                                this.open_place_menu(Target::Guild(guild), event.position, cx)
                            }),
                        )
                    }),
            )
            // Mention count in the icon's corner, ringed with the background.
            .when(mentions > 0, |space| {
                space.child(
                    div()
                        .absolute()
                        .bottom(px(-4.))
                        .right(px(10.))
                        .rounded_full()
                        .border_4()
                        .border_color(rgb(theme::BASE_LOWEST))
                        .child(widgets::badge(mentions as usize)),
                )
            })
    }
}
