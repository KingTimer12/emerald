//! The member list's Activity section: what members of the server did
//! lately (games played, top artists…), from the content inventory feed,
//! three cards at first.

use super::*;
use super::super::account::now_ms;
use emerald_discord::model::{Content, content_type, trait_type};

/// Cards shown before "Show all".
const FOLDED: usize = 3;

impl Emerald {
    /// Asks for the feed when it is due and the window is in front, as the
    /// web client does on focus.
    pub(in super::super) fn fetch_activity(&mut self) {
        if self.connection.connected && self.activity_feed.should_fetch(now_ms(), self.window_active) {
            let refresh = self.activity_feed.asked();
            self.net.post(Command::ContentInventory { refresh });
        }
    }

    pub(in super::super) fn on_activity_feed(&mut self, feed: Result<emerald_discord::model::Feed, String>) {
        match feed {
            Ok(feed) => self.activity_feed.loaded(feed, now_ms()),
            Err(error) => {
                tracing::warn!("activity feed: {error}");
                self.activity_feed.failed(now_ms());
            }
        }
    }

    /// The section, when the server shows it and anyone here did anything.
    pub(super) fn render_activity(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let space = self.spaces.current();
        let info = space.info()?;
        let channel = self.selected_channel?;
        let forum = self.spaces.channel(channel).is_some_and(|c| c.kind.is_forum());
        let count = self.members.current().map_or(0, |list| list.member_count);
        if forum || !info.shows_activity(count) {
            return None;
        }
        // Known members stand in for "members who can see the channel".
        let entries = self.activity_feed.for_guild(|author| info.member(author).is_some());
        if entries.is_empty() {
            return None;
        }
        let total = entries.len();
        let shown = if self.activity_expanded { total } else { total.min(FOLDED) };
        Some(
            div()
                .w_full()
                .flex_none()
                .px_2()
                .pb_2()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .px_2()
                        .pt_2()
                        .text_size(zoomed(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(theme::channels_default()))
                        .child(t!("activity_feed.title")),
                )
                .children(entries.into_iter().take(shown).enumerate().map(|(ix, entry)| self.activity_card(ix, entry)))
                .when(total > FOLDED, |section| {
                    let label = if self.activity_expanded { t!("activity_feed.show_less") } else { t!("activity_feed.show_all") };
                    section.child(
                        div().px_2().child(widgets::link_button("activity-more", label).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.activity_expanded = !this.activity_expanded;
                                cx.notify();
                            },
                        ))),
                    )
                })
                .into_any_element(),
        )
    }

    fn activity_card(&self, ix: usize, entry: &Content) -> impl IntoElement {
        let author = entry.author_id;
        let user = self.people.user(author);
        let name: SharedString = self
            .spaces
            .current()
            .info()
            .and_then(|g| g.member(author))
            .and_then(|m| m.nick.clone())
            .or_else(|| user.map(|u| u.display_name().to_owned()))
            .unwrap_or_default()
            .into();
        div()
            .id(("activity-card", ix))
            .flex()
            .items_center()
            .gap_2()
            .p_2()
            .rounded(theme::radius::card())
            .bg(rgb(theme::surface_high()))
            .border_1()
            .border_color(theme::border_normal())
            .child(widgets::avatar(&name, author, self.user_avatar(author), 32., None))
            .child(
                div()
                    .flex_1()
                    .min_w(zoomed(0.))
                    .flex()
                    .flex_col()
                    .child(div().truncate().text_size(zoomed(14.)).font_weight(FontWeight::MEDIUM).text_color(rgb(theme::text_strong())).child(name))
                    .child(div().truncate().text_size(zoomed(12.)).text_color(rgb(theme::text_muted())).child(card_line(entry))),
            )
    }
}

/// What a card says the member did. Only "Top Artist" is Discord's
/// wording; the others are a guess, as are the fields naming the subject.
fn card_line(entry: &Content) -> String {
    let name = entry.subject().unwrap_or_default();
    let live = entry.trait_value(trait_type::IS_LIVE, "is_live").and_then(|v| v.as_bool()) == Some(true);
    match entry.content_type {
        content_type::PLAYED_GAME | content_type::LAUNCHED_ACTIVITY if live => t!("activity_feed.playing", name = name),
        content_type::PLAYED_GAME | content_type::LAUNCHED_ACTIVITY => t!("activity_feed.played", name = name),
        content_type::WATCHED_MEDIA => t!("activity_feed.watched", name = name),
        content_type::TOP_GAME => t!("activity_feed.top_game", name = name),
        content_type::TOP_ARTIST => {
            let count = entry.trait_value(trait_type::AGGREGATE_COUNT, "count").and_then(|v| v.as_u64()).unwrap_or(0);
            t!("activity_feed.top_artist", count = count)
        }
        content_type::LISTENED_SESSION => t!("activity_feed.listened", name = name),
        _ => t!("activity_feed.something").to_owned(),
    }
}
