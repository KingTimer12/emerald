//! A text channel's threads: the active ones the gateway knows, then the
//! archived ones paged in from Discord, or those whose name matches a
//! search.

use super::*;

use super::super::super::time;
use emerald_discord::model::Channel;
use emerald_discord::{Command, ThreadPage, ThreadQuery};
use crate::zoom::zoomed;

pub(in super::super) struct ThreadsPanel {
    pub channel: Snowflake,
    query: ThreadQuery,
    /// Threads the search returned, in its order.
    loaded: Vec<Snowflake>,
    more: bool,
    loading: bool,
}

impl Emerald {
    pub(in super::super) fn toggle_threads(&mut self, channel: Snowflake, cx: &mut Context<Self>) {
        if self.panel_is(PanelKind::Threads) {
            return self.close_panel(cx);
        }
        self.thread_query.update(cx, |input, cx| input.set_text("", cx));
        self.load_threads(channel, ThreadQuery::default(), cx);
    }

    /// Enter in the panel's search box: threads named like `text`, or the
    /// archived ones again when it is empty.
    pub(in super::super) fn search_threads(&mut self, text: String, cx: &mut Context<Self>) {
        let Some(Panel::Threads(threads)) = &self.panel else { return };
        let name = text.trim();
        let query = ThreadQuery {
            name: (!name.is_empty()).then(|| name.to_owned()),
            ..ThreadQuery::default()
        };
        self.load_threads(threads.channel, query, cx);
    }

    fn load_threads(&mut self, channel: Snowflake, query: ThreadQuery, cx: &mut Context<Self>) {
        self.net.post(Command::ThreadSearch {
            channel,
            query: query.clone(),
            offset: 0,
        });
        self.panel = Some(Panel::Threads(ThreadsPanel {
            channel,
            query,
            loaded: Vec::new(),
            more: false,
            loading: true,
        }));
        cx.notify();
    }

    fn more_threads(&mut self, cx: &mut Context<Self>) {
        let Some(Panel::Threads(threads)) = &mut self.panel else { return };
        if threads.loading || !threads.more {
            return;
        }
        threads.loading = true;
        self.net.post(Command::ThreadSearch {
            channel: threads.channel,
            query: threads.query.clone(),
            offset: threads.loaded.len(),
        });
        cx.notify();
    }

    /// A page for the panel; false when the panel did not ask for it.
    pub(in super::super) fn on_thread_panel_page(
        &mut self,
        channel: Snowflake,
        query: &ThreadQuery,
        offset: usize,
        page: &mut Option<ThreadPage>,
    ) -> bool {
        let Some(Panel::Threads(threads)) = &mut self.panel else { return false };
        if threads.channel != channel || threads.query != *query || threads.loaded.len() != offset {
            return false;
        }
        threads.loading = false;
        let Some(page) = page.take() else {
            threads.more = false;
            return true;
        };
        threads.more = page.has_more && !page.threads.is_empty();
        threads.loaded.extend(page.threads.iter().map(|t| t.id));
        if let Some(guild) = self.spaces.guild_id_of(channel)
            && let Some(space) = self.spaces.get_mut(guild)
        {
            space.add_threads(page.threads);
        }
        true
    }

    pub(super) fn render_threads(&self, threads: &ThreadsPanel, cx: &mut Context<Self>) -> AnyElement {
        let space = self.spaces.current();
        let find = |id: Snowflake| space.channels().iter().find(|c| c.id == id);
        let searching = threads.query.name.is_some();
        // Without a search: the active ones first, newest activity on top.
        let mut active: Vec<&Channel> = if searching {
            Vec::new()
        } else {
            space
                .channels()
                .iter()
                .filter(|c| c.parent_id == Some(threads.channel) && c.is_active_thread())
                .collect()
        };
        active.sort_by_key(|c| std::cmp::Reverse(c.activity()));
        let loaded: Vec<&Channel> = threads
            .loaded
            .iter()
            .filter_map(|id| find(*id))
            .filter(|c| searching || !c.is_active_thread())
            .collect();
        let section = |label: &'static str| {
            div()
                .px_2()
                .pt_3()
                .pb_1()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::muted())
                .child(label)
        };
        let mut rows: Vec<AnyElement> = Vec::new();
        if !active.is_empty() {
            rows.push(section(t!("threads.active")).into_any_element());
            rows.extend(active.iter().enumerate().map(|(ix, t)| self.render_thread_row(ix, t, cx)));
        }
        if !loaded.is_empty() {
            let label = if searching { t!("threads.found") } else { t!("threads.archived") };
            rows.push(section(label).into_any_element());
            rows.extend(loaded.iter().enumerate().map(|(ix, t)| self.render_thread_row(active.len() + ix, t, cx)));
        }
        let footer = if threads.loading {
            Some(note(t!("common.loading")))
        } else if rows.is_empty() {
            Some(note(t!("threads.empty")))
        } else {
            None
        };
        div()
            .flex_1()
            .min_h(zoomed(0.))
            .flex()
            .flex_col()
            .child(
                div()
                    .m_3()
                    .h(zoomed(32.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_1()
                    .rounded(theme::radius::control())
                    .bg(rgb(theme::base_lowest()))
                    .child(icons::icon(icons::SEARCH, 16., theme::interactive()))
                    .child(div().flex_1().min_w(zoomed(0.)).child(self.thread_query.clone())),
            )
            .child(
                div()
                    .id("threads-list")
                    .flex_1()
                    .min_h(zoomed(0.))
                    .overflow_y_scroll()
                    .px_2()
                    .pb_2()
                    .flex()
                    .flex_col()
                    .children(rows)
                    .children(footer)
                    .when(threads.more && !threads.loading, |list| {
                        list.child(
                            div().p_3().flex().justify_center().child(
                                widgets::link_button("threads-more", t!("panel.load_more"))
                                    .on_click(cx.listener(|this, _, _, cx| this.more_threads(cx))),
                            ),
                        )
                    }),
            )
            .into_any_element()
    }

    fn render_thread_row(&self, ix: usize, thread: &Channel, cx: &mut Context<Self>) -> AnyElement {
        let opened = thread.clone();
        let count = thread.message_count.unwrap_or(0);
        div()
            .id(("thread-row", ix))
            .px_3()
            .py_2()
            .flex()
            .flex_col()
            .gap(zoomed(2.))
            .rounded(theme::radius::control())
            .cursor_pointer()
            .hover(|s| s.bg(theme::hover()))
            .on_click(cx.listener(move |this, _, window, cx| this.open_thread(opened.clone(), window, cx)))
            .child(
                div()
                    .truncate()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::text_strong()))
                    .child(thread.name.clone().unwrap_or_default()),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .text_xs()
                    .text_color(theme::muted())
                    .child(icons::icon(icons::THREAD, 14., theme::text_muted()))
                    .child(count.to_string())
                    .child("·")
                    .child(time::short_date(thread.activity().unix_secs())),
            )
            .into_any_element()
    }
}
