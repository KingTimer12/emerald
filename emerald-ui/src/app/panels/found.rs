//! Messages listed in a panel: one card each, naming where it was sent,
//! that jumps to the message when clicked.

use super::*;

use emerald_discord::model::Message;
use crate::zoom::zoomed;

impl Found {
    /// `message` as a card; its guild from the message, else from where
    /// the client lists the channel.
    pub(super) fn new(message: Message, spaces: &super::super::state::Spaces) -> Self {
        let channel = message.channel_id;
        let guild = message.guild_id.or_else(|| spaces.guild_id_of(channel));
        Self {
            msg: Msg::from(message),
            channel,
            guild,
        }
    }
}

impl Emerald {
    /// The results, then a loader or a "load more" link, or why there are
    /// none. `more` asks for the next page.
    pub(super) fn render_results(
        &self,
        id: &'static str,
        results: &Results,
        empty: &'static str,
        show_place: bool,
        more: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let footer = if results.loading {
            Some(note(t!("common.loading")).into_any_element())
        } else if let Some(error) = &results.error {
            Some(note(error.clone()).text_color(rgb(theme::feedback_critical())).into_any_element())
        } else if results.items.is_empty() {
            Some(note(empty).into_any_element())
        } else if results.more {
            Some(
                div()
                    .p_3()
                    .flex()
                    .justify_center()
                    .child(
                        widgets::link_button((id, usize::MAX), t!("panel.load_more"))
                            .on_click(cx.listener(move |this, _, _, cx| more(this, cx))),
                    )
                    .into_any_element(),
            )
        } else {
            None
        };
        div()
            .id(id)
            .flex_1()
            .min_h(zoomed(0.))
            .overflow_y_scroll()
            .p_2()
            .flex()
            .flex_col()
            .gap_2()
            .children(results.items.iter().enumerate().map(|(ix, found)| self.render_found(id, ix, found, show_place, cx)))
            .children(footer)
            .into_any_element()
    }

    fn render_found(&self, id: &'static str, ix: usize, found: &Found, show_place: bool, cx: &mut Context<Self>) -> AnyElement {
        let msg = &found.msg;
        let (author, color) = self.author_style(msg);
        let (guild, channel, message) = (found.guild, found.channel, msg.id);
        let place = show_place.then(|| self.place_label(guild, channel));
        let attachments = msg.images.len() + msg.files.len();
        div()
            .id((id, ix))
            .flex()
            .flex_col()
            .gap_1()
            .p_3()
            .rounded_lg()
            .border_1()
            .border_color(theme::border())
            .bg(rgb(theme::base_low()))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(theme::surface_high())))
            .on_click(cx.listener(move |this, _, window, cx| this.open_channel_link(guild, channel, Some(message), window, cx)))
            .children(place.map(|place| {
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::muted())
                    .truncate()
                    .child(place)
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(widgets::avatar(&author, msg.author_id, self.avatar_image(&msg.avatar_url), 24., None))
                    .child(div().text_color(rgb(color)).font_weight(FontWeight::MEDIUM).text_size(zoomed(14.)).child(author))
                    .child(div().text_xs().text_color(theme::muted()).child(msg.time.clone())),
            )
            .child(
                div()
                    .text_size(zoomed(14.))
                    .text_color(rgb(theme::text_default()))
                    .child(self.rich_text((id, ix), &msg.content, &msg.mentions)),
            )
            .when(attachments > 0, |card| {
                card.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .text_xs()
                        .text_color(theme::muted())
                        .child(icons::icon(icons::ATTACH, 14., theme::text_muted()))
                        .child(t!("panel.attachments", count = attachments)),
                )
            })
            .into_any_element()
    }

    /// "#channel · Server", or the DM's name.
    pub(super) fn place_label(&self, guild: Option<Snowflake>, channel: Snowflake) -> SharedString {
        let name = self.spaces.iter().find_map(|space| space.row(channel).map(|row| row.name.clone()));
        let name = name.or_else(|| self.spaces.channel(channel).and_then(|c| c.name.clone()).map(Into::into));
        let name = name.unwrap_or_else(|| "?".into());
        match guild.and_then(|g| self.spaces.iter().find(|s| s.id == g)) {
            Some(server) => format!("#{name} · {}", server.name).into(),
            None => name,
        }
    }
}
