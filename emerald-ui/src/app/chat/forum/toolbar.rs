//! The bar above a forum's posts: search by name, sort, the tag filter and
//! "New Post". Each change pages the posts in again from the start.

use super::*;
use crate::zoom::zoomed;

impl Emerald {
    /// Changes the forum's sort, tags or search, and pages again.
    fn refilter_forum(&mut self, forum: Snowflake, change: impl FnOnce(&mut ThreadQuery), cx: &mut Context<Self>) {
        let Some(state) = self.forums.get_mut(&forum) else { return };
        let mut query = state.query.clone();
        change(&mut query);
        if query != state.query {
            state.restart(query);
            cx.notify();
        }
    }

    /// Enter in the forum's search box; empty shows every post again.
    pub(in super::super::super) fn search_forum(&mut self, text: String, cx: &mut Context<Self>) {
        let Some(forum) = self.selected_channel.filter(|f| self.forums.contains_key(f)) else { return };
        let name = text.trim().to_owned();
        self.refilter_forum(forum, |query| query.name = (!name.is_empty()).then_some(name), cx);
    }

    /// Search, sort, tag filter and "New Post" above the posts.
    pub(super) fn render_forum_toolbar(&self, forum: Snowflake, state: &Forum, cx: &mut Context<Self>) -> gpui::Div {
        let postable = self.can_send(forum);
        let available = self
            .spaces
            .current()
            .channels()
            .iter()
            .find(|c| c.id == forum)
            .map_or(&[][..], |f| f.available_tags.as_slice());
        let sort = if state.query.by_creation { t!("forum.sort_created") } else { t!("forum.sort_activity") };
        div()
            .flex()
            .flex_col()
            .gap_2()
            .px_4()
            .pt_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .h(zoomed(32.))
                            .px_2()
                            .flex()
                            .items_center()
                            .gap_1()
                            .rounded(theme::radius::control())
                            .bg(rgb(theme::base_lowest()))
                            .child(icon(icons::SEARCH, 16., theme::interactive()))
                            .child(div().flex_1().min_w(zoomed(0.)).child(self.forum_query.clone())),
                    )
                    .child(
                        chip(("forum-sort", 0), sort.into(), false)
                            .tooltip(widgets::tooltip(t!("forum.sort")))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.refilter_forum(forum, |query| query.by_creation = !query.by_creation, cx)
                            })),
                    )
                    .when(postable, |row| {
                        row.child(
                            widgets::button("new-post", t!("forum.new_post"), widgets::ButtonKind::Brand)
                                .h(zoomed(32.))
                                .on_click(cx.listener(move |this, _, window, cx| this.open_create_post(forum, window, cx))),
                        )
                    }),
            )
            .when(!available.is_empty(), |toolbar| {
                toolbar.child(div().flex().flex_wrap().gap_1().children(available.iter().enumerate().map(|(ix, t)| {
                    let id = t.id;
                    let label = match &t.emoji_name {
                        Some(emoji) => format!("{emoji} {}", t.name),
                        None => t.name.clone(),
                    };
                    let toggle = move |query: &mut ThreadQuery| match query.tags.iter().position(|t| *t == id) {
                        Some(at) => {
                            query.tags.remove(at);
                        }
                        None => query.tags.push(id),
                    };
                    chip(("forum-tag", ix), label.into(), state.query.tags.contains(&id))
                        .on_click(cx.listener(move |this, _, _, cx| this.refilter_forum(forum, toggle, cx)))
                })))
            })
    }
}

/// A toolbar choice: the sort, or a tag to filter by, lit when `on`.
fn chip(id: (&'static str, usize), label: SharedString, on: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px_2()
        .py(zoomed(2.))
        .rounded_full()
        .border_1()
        .border_color(if on { rgb(theme::brand()) } else { theme::border() })
        .bg(if on { theme::active() } else { rgb(theme::base_lower()) })
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(if on { theme::text_strong() } else { theme::text_default() }))
        .cursor_pointer()
        .hover(|s| s.bg(theme::hover()))
        .child(label)
}
