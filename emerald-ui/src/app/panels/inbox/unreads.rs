//! The Inbox's Unreads tab: a tip until dismissed, then one card per
//! channel with unread messages, holding those messages.

use super::*;
use super::super::super::chat::day_separator;
use crate::time;

impl Emerald {
    /// Unread channels outside muted places, servers in list order.
    fn unread_channels(&self) -> Vec<(Option<Snowflake>, Snowflake)> {
        let now = crate::time::now_secs();
        self.spaces
            .iter()
            .flat_map(|space| {
                let guild = (!space.is_dms()).then_some(space.id);
                space
                    .readable()
                    .filter(|c| self.reads.is_unread(c.id))
                    .filter(move |c| !self.notify.muted(&self.place_of(guild, c.id), now))
                    .map(move |c| (guild, c.id))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// Asks for the messages of the cards shown that have none yet.
    pub(super) fn load_unreads(&mut self) {
        let channels = self.unread_channels();
        let Some(inbox) = &mut self.inbox else { return };
        for (_, channel) in channels.into_iter().take(inbox.shown) {
            if inbox.unreads.contains_key(&channel) {
                continue;
            }
            inbox.unreads.insert(channel, Results::loading());
            self.net.post(Command::Unread { channel, after: self.reads.last_read(channel) });
        }
    }

    pub(in super::super::super) fn on_unread(&mut self, channel: Snowflake, messages: Result<Vec<Message>, String>) {
        let Some(results) = self.inbox.as_mut().and_then(|inbox| inbox.unreads.get_mut(&channel)) else { return };
        results.loading = false;
        match messages {
            Ok(messages) => {
                let spaces = &self.spaces;
                results.items = messages.into_iter().map(|message| Found::new(message, spaces)).collect();
            }
            Err(error) => results.error = Some(error.into()),
        }
    }

    /// Marks one card's channel read, and loads the card taking its place.
    fn mark_card_read(&mut self, channel: Snowflake, cx: &mut Context<Self>) {
        self.mark_channel_read(channel, cx);
        self.load_unreads();
    }

    /// The check beside the title: every unread channel, in one receipt.
    pub(super) fn mark_inbox_read(&mut self, cx: &mut Context<Self>) {
        let channels: Vec<(Snowflake, Snowflake)> = self
            .unread_channels()
            .into_iter()
            .filter_map(|(_, channel)| Some((channel, self.newest_message(channel)?)))
            .collect();
        for (channel, message) in &channels {
            self.reads.acked(*channel, *message, 0);
        }
        if !channels.is_empty() {
            self.net.post(Command::AckBulk { channels });
        }
        cx.notify();
    }

    fn more_unreads(&mut self, cx: &mut Context<Self>) {
        if let Some(inbox) = &mut self.inbox {
            inbox.shown += UNREADS_PAGE;
        }
        self.load_unreads();
        cx.notify();
    }

    fn toggle_card(&mut self, channel: Snowflake, cx: &mut Context<Self>) {
        let Some(inbox) = &mut self.inbox else { return };
        if !inbox.collapsed.remove(&channel) {
            inbox.collapsed.insert(channel);
        }
        cx.notify();
    }

    pub(super) fn render_unreads(&self, inbox: &Inbox, cx: &mut Context<Self>) -> AnyElement {
        let unread = self.unread_channels();
        let more = unread.len() > inbox.shown;
        let tip = (!self.settings.inbox_tip_dismissed).then(|| self.render_inbox_tip(cx));
        let empty = unread.is_empty().then(|| note(t!("inbox.no_unreads")));
        div()
            .id("unreads")
            .flex_1()
            .min_h(zoomed(0.))
            .overflow_y_scroll()
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            .children(tip)
            .children(empty)
            .children(
                unread
                    .into_iter()
                    .take(inbox.shown)
                    .enumerate()
                    .map(|(ix, (guild, channel))| self.render_unread_card(inbox, ix, guild, channel, cx)),
            )
            .when(more, |list| {
                list.child(
                    div().flex().justify_center().child(
                        widgets::link_button("unreads-more", t!("panel.load_more"))
                            .on_click(cx.listener(|this, _, _, cx| this.more_unreads(cx))),
                    ),
                )
            })
            .into_any_element()
    }

    /// What the tab is for, until "Got it!" puts it away for good.
    fn render_inbox_tip(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex_none()
            .p_4()
            .flex()
            .gap_4()
            .rounded(theme::radius::card())
            .bg(rgb(theme::base_low()))
            .child(
                div()
                    .size(zoomed(40.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(theme::radius::card())
                    .bg(rgb(theme::surface_high()))
                    .child(icons::icon(icons::INBOX, 24., theme::text_strong())),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::text_strong()))
                            .child(t!("inbox.tip_title")),
                    )
                    .child(div().text_size(zoomed(14.)).text_color(rgb(theme::text_default())).child(t!("inbox.tip_body")))
                    .child(
                        div().mt_1().flex().child(
                            widgets::button("inbox-tip-ok", t!("inbox.tip_ok"), widgets::ButtonKind::Brand)
                                .h(zoomed(32.))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.settings.inbox_tip_dismissed = true;
                                    this.save_settings_soon(cx);
                                    cx.notify();
                                })),
                        ),
                    ),
            )
            .into_any_element()
    }

    fn render_unread_card(
        &self,
        inbox: &Inbox,
        ix: usize,
        guild: Option<Snowflake>,
        channel: Snowflake,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let collapsed = inbox.collapsed.contains(&channel);
        let space = guild.and_then(|guild| self.spaces.iter().find(|space| space.id == guild));
        let row = self.spaces.iter().find_map(|space| space.row(channel));
        let name: SharedString = row.map(|row| row.name.clone()).unwrap_or_else(|| "?".into());
        let category = self
            .spaces
            .channel(channel)
            .and_then(|c| c.parent_id)
            .and_then(|parent| self.spaces.channel(parent))
            .and_then(|parent| parent.name.clone());
        let subtitle: Option<SharedString> = space.map(|space| match category {
            Some(category) => format!("{} \u{203a} {category}", space.name).into(),
            None => space.name.clone(),
        });
        let picture = match space {
            Some(space) => {
                let tile = div()
                    .size(zoomed(32.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .overflow_hidden()
                    .rounded(theme::radius::server_icon_at(32.))
                    .bg(rgb(theme::base_low()))
                    .text_size(zoomed(12.))
                    .text_color(rgb(theme::text_strong()));
                match space.icon_url.as_ref().and_then(|url| self.avatar_image(url)) {
                    Some(image) => tile.child(gpui::img(image).size_full()),
                    None => tile.child(space.initials.clone()),
                }
            }
            None => {
                let image = row.and_then(|row| row.picture.clone()).and_then(|url| self.avatar_image(&url));
                widgets::avatar(&name, channel, image, 32., None)
            }
        };
        let title = match guild {
            Some(_) => format!("# {name}"),
            None => name.to_string(),
        };
        let bell = guild.map(|guild| {
            let tracked = popover::Tracked::default();
            let muted = self.notify.channel_muted(Some(guild), channel, crate::time::now_secs());
            let place = self.place_of(Some(guild), channel);
            let glyph = super::super::super::place_menu::bell_icon(muted, self.notify.resolved(&place));
            tracked.track(square_button(("unread-bell", ix), glyph)).on_click(cx.listener(move |this, _, _, cx| {
                this.open_notify_menu(guild, channel, Anchor::below_end(tracked.bounds()), cx)
            }))
        });
        let header = div()
            .p_3()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .id(("unread-open", ix))
                    .flex_1()
                    .min_w(zoomed(0.))
                    .flex()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .child(picture)
                    .child(icons::icon(icons::CHEVRON_RIGHT, 14., theme::text_muted()))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .min_w(zoomed(0.))
                            .child(
                                div()
                                    .truncate()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(theme::text_strong()))
                                    .child(title),
                            )
                            .children(subtitle.map(|subtitle| {
                                div().truncate().text_xs().text_color(rgb(theme::text_muted())).child(subtitle)
                            })),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.close_inbox(cx);
                        this.open_channel_link(guild, channel, None, window, cx);
                    })),
            )
            .children(bell)
            .child(
                square_button(("unread-read", ix), icons::CHECK)
                    .tooltip(widgets::tooltip(t!("read.mark_read")))
                    .on_click(cx.listener(move |this, _, _, cx| this.mark_card_read(channel, cx))),
            )
            .child(
                div()
                    .id(("unread-fold", ix))
                    .size(zoomed(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .child(
                        icons::icon(icons::CHEVRON, 16., theme::interactive())
                            .when(collapsed, |icon| icon.with_transformation(gpui::Transformation::rotate(gpui::radians(-std::f32::consts::FRAC_PI_2)))),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_card(channel, cx))),
            );
        let body = (!collapsed).then(|| self.render_card_messages(inbox.unreads.get(&channel), ix, cx));
        div()
            .flex_none()
            .flex()
            .flex_col()
            .rounded(theme::radius::card())
            .border_1()
            .border_color(theme::border_normal())
            .bg(rgb(theme::surface_high()))
            .child(header)
            .children(body)
            .into_any_element()
    }

    /// The card's messages as in the chat, a day line before each new day.
    fn render_card_messages(&self, results: Option<&Results>, card: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(results) = results else { return div().into_any_element() };
        if results.loading {
            return note(t!("common.loading")).into_any_element();
        }
        if let Some(error) = &results.error {
            return note(error.clone()).text_color(rgb(theme::feedback_critical())).into_any_element();
        }
        let mut column = div().flex().flex_col().pb_3().border_t_1().border_color(theme::border_normal());
        let mut day = None;
        for (ix, found) in results.items.iter().enumerate() {
            let msg = &found.msg;
            if day != Some(msg.day) {
                day = Some(msg.day);
                column = column.child(day_separator(time::day_label(msg.secs)));
            }
            let (author, color) = self.author_style(msg);
            let (guild, channel, message) = (found.guild, found.channel, msg.id);
            column = column.child(
                div()
                    .id(("unread-message", card * 1000 + ix))
                    .px_4()
                    .py_1()
                    .flex()
                    .gap_3()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme::hover()))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.close_inbox(cx);
                        this.open_channel_link(guild, channel, Some(message), window, cx);
                    }))
                    .child(widgets::avatar(&author, msg.author_id, self.avatar_image(&msg.avatar_url), 40., None))
                    .child(
                        div()
                            .flex_1()
                            .min_w(zoomed(0.))
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .flex()
                                    .items_baseline()
                                    .gap_2()
                                    .child(div().font_weight(FontWeight::MEDIUM).text_color(rgb(color)).child(author))
                                    .child(div().text_xs().text_color(rgb(theme::text_muted())).child(msg.time.clone())),
                            )
                            .child(
                                div()
                                    .text_color(rgb(theme::text_default()))
                                    .child(self.rich_text(("unread-text", card * 1000 + ix), &msg.content, &msg.mentions)),
                            ),
                    ),
            );
        }
        column.into_any_element()
    }
}
