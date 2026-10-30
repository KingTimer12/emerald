//! Forum and media channels: their posts as cards, each opening its thread.
//! Active posts come from the gateway; archived ones page in from the
//! thread search as the list reaches its end, and first messages load in
//! batches for the cards being drawn, as the web client does.

use rustc_hash::{FxHashMap, FxHashSet};

use gpui::{AnyElement, Context, FontWeight, ListAlignment, ListState, SharedString, div, img, list, prelude::*, rgb};

use super::super::super::icons::{self, icon};
use super::super::super::{theme, time};
use super::super::{Emerald, widgets};
use super::super::state::Msg;
use emerald_discord::model::{Channel, Message, Snowflake};
use emerald_discord::{Command, ThreadPage, ThreadQuery};
use crate::zoom::zoomed;

/// First messages asked for at once, as the web client batches them.
const POST_BATCH: usize = 10;
/// Discord's flag on a post pinned to the top of its forum.
const PINNED: u64 = 1 << 1;
const THUMBNAIL: f32 = 72.;

mod toolbar;

/// What this client knows of one forum beyond its active posts.
pub(in super::super) struct Forum {
    /// First messages, once Discord answered for the post.
    first: FxHashMap<Snowflake, Msg>,
    asked: FxHashSet<Snowflake>,
    /// Posts loaded from the thread search so far, and whether there are
    /// more: archived ones, or the matches of a name search.
    archived: usize,
    more: bool,
    loading: bool,
    /// The sort, tag filter and name search in effect.
    query: ThreadQuery,
    /// Posts a name search found, in its order.
    found: Vec<Snowflake>,
    list: ListState,
}

impl Forum {
    fn new(by_creation: bool) -> Self {
        Self {
            first: FxHashMap::default(),
            asked: FxHashSet::default(),
            archived: 0,
            more: true,
            loading: false,
            query: ThreadQuery {
                by_creation,
                ..ThreadQuery::default()
            },
            found: Vec::new(),
            list: ListState::new(0, ListAlignment::Top, zoomed(400.)),
        }
    }

    /// Starts paging again for a new sort, filter or search.
    fn restart(&mut self, query: ThreadQuery) {
        self.query = query;
        self.archived = 0;
        self.more = true;
        self.loading = false;
        self.found.clear();
        self.list.reset(0);
    }
}

impl Emerald {
    /// The forum was opened.
    pub(in super::super) fn open_forum(&mut self, forum: Snowflake, cx: &mut Context<Self>) {
        let by_creation = self.spaces.current().channels().iter().any(|c| c.id == forum && c.default_sort_order == Some(1));
        let state = self.forums.entry(forum).or_insert_with(|| Forum::new(by_creation));
        let name = state.query.name.clone().unwrap_or_default();
        self.forum_query.update(cx, |input, cx| input.set_text(name, cx));
        cx.notify();
    }

    /// The forum's posts as the web client lists them: pinned first, then
    /// by latest activity or by creation; only those with a picked tag, or
    /// those a name search found.
    fn forum_posts(&self, forum: Snowflake) -> Vec<&Channel> {
        let space = self.spaces.current();
        let Some(state) = self.forums.get(&forum) else { return Vec::new() };
        let by_creation = state.query.by_creation;
        let tags = &state.query.tags;
        let searching = state.query.name.is_some();
        let mut posts: Vec<&Channel> = space
            .channels()
            .iter()
            .filter(|c| c.kind.is_thread() && c.parent_id == Some(forum))
            .filter(|c| !searching || state.found.contains(&c.id))
            .filter(|c| tags.is_empty() || c.applied_tags.iter().any(|t| tags.contains(t)))
            .collect();
        posts.sort_by_key(|c| {
            let pinned = c.flags & PINNED != 0;
            let order = if by_creation { c.id } else { c.activity() };
            (std::cmp::Reverse(pinned), std::cmp::Reverse(order))
        });
        posts
    }

    pub(in super::super) fn on_forum_posts(&mut self, forum: Snowflake, first: Vec<(Snowflake, Message)>) {
        let Some(state) = self.forums.get_mut(&forum) else { return };
        for (thread, message) in first {
            state.first.insert(thread, Msg::from(message));
        }
    }

    /// A page of the forum's posts from the thread search.
    pub(in super::super) fn on_forum_page(&mut self, forum: Snowflake, query: ThreadQuery, offset: usize, page: Option<ThreadPage>) {
        let Some(state) = self.forums.get_mut(&forum) else { return };
        if offset != state.archived || query != state.query {
            return;
        }
        state.loading = false;
        let Some(page) = page else {
            state.more = false;
            return;
        };
        state.archived += page.threads.len();
        state.more = page.has_more && !page.threads.is_empty();
        if state.query.name.is_some() {
            state.found.extend(page.threads.iter().map(|t| t.id));
        }
        for message in page.first {
            state.asked.insert(message.channel_id);
            state.first.insert(message.channel_id, Msg::from(message));
        }
        let guild = self.spaces.current().id;
        if let Some(space) = self.spaces.get_mut(guild) {
            space.add_threads(page.threads);
        }
    }

    /// Asks for the first messages of the posts from `ix` on that have none.
    fn request_posts(&mut self, forum: Snowflake, ix: usize) {
        let missing: Vec<Snowflake> = {
            let Some(state) = self.forums.get(&forum) else { return };
            self.forum_posts(forum)
                .iter()
                .skip(ix)
                .map(|c| c.id)
                .filter(|id| !state.asked.contains(id))
                .take(POST_BATCH)
                .collect()
        };
        if missing.is_empty() {
            return;
        }
        if let Some(state) = self.forums.get_mut(&forum) {
            state.asked.extend(missing.iter().copied());
        }
        self.net.post(Command::ForumPosts { forum, threads: missing });
    }

    /// The next page of archived posts, once the list reaches its end.
    fn request_archived(&mut self, forum: Snowflake) {
        let Some(state) = self.forums.get_mut(&forum) else { return };
        if !state.more || state.loading {
            return;
        }
        state.loading = true;
        self.net.post(Command::ThreadSearch {
            channel: forum,
            query: state.query.clone(),
            offset: state.archived,
        });
    }


    /// The post list, in place of messages.
    pub(super) fn render_forum(&self, forum: Snowflake, cx: &mut Context<Self>) -> AnyElement {
        let Some(state) = self.forums.get(&forum) else { return div().into_any_element() };
        let count = self.forum_posts(forum).len();
        // One more row at the end: the archive loader, or nothing.
        if state.list.item_count() != count + 1 {
            state.list.reset(count + 1);
        }
        let posts = list(
            state.list.clone(),
            cx.processor(move |this, ix: usize, _, cx| {
                if ix >= this.forum_posts(forum).len() {
                    this.request_archived(forum);
                    return this.render_forum_end(forum).into_any_element();
                }
                this.request_posts(forum, ix);
                this.render_post(forum, ix, cx)
            }),
        )
        .flex_1()
        .px_4()
        .py_2();
        div()
            .flex_1()
            .min_h(zoomed(0.))
            .flex()
            .flex_col()
            .child(self.render_forum_toolbar(forum, state, cx))
            .child(posts)
            .into_any_element()
    }

    fn render_forum_end(&self, forum: Snowflake) -> gpui::Div {
        let state = self.forums.get(&forum);
        let text = if state.is_some_and(|s| s.loading) {
            Some(t!("common.loading"))
        } else if self.forum_posts(forum).is_empty() {
            Some(t!("forum.empty"))
        } else {
            None
        };
        div()
            .py_4()
            .flex()
            .justify_center()
            .text_color(theme::muted())
            .children(text)
    }

    /// One card: title, tags, the opening message and its first image, and
    /// how many messages followed.
    fn render_post(&self, forum: Snowflake, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let posts = self.forum_posts(forum);
        let Some(post) = posts.get(ix) else { return div().into_any_element() };
        let id = post.id;
        let first = self.forums.get(&forum).and_then(|s| s.first.get(&id));
        let tags = self.spaces.current().channels().iter().find(|c| c.id == forum).map_or(Vec::new(), |f| {
            f.available_tags.iter().filter(|t| post.applied_tags.contains(&t.id)).collect()
        });
        let preview = first.map(|msg| {
            let (author, color) = self.author_style(msg);
            // One line, as the web client's card shows the opening message.
            let content = msg.content.split_whitespace().collect::<Vec<_>>().join(" ");
            div()
                .flex()
                .gap_1()
                .text_size(zoomed(14.))
                .child(div().flex_none().text_color(rgb(color)).font_weight(FontWeight::MEDIUM).child(format!("{author}:")))
                .child(
                    div()
                        .flex_1()
                        .min_w(zoomed(0.))
                        .truncate()
                        .text_color(rgb(theme::text_default()))
                        .child(self.rich_text(("post-preview", ix), &content, &msg.mentions)),
                )
        });
        let thumbnail = first.and_then(|msg| msg.images.first()).filter(|p| !p.spoiler).map(|picture| {
            let frame = div().flex_none().size(zoomed(THUMBNAIL)).rounded(theme::radius::card()).overflow_hidden();
            match self.avatar_image(&picture.url) {
                Some(image) => frame.child(img(image).size_full()),
                None => frame.bg(theme::hover()),
            }
        });
        let count = post.message_count.unwrap_or(0);
        let activity = time::short_date(post.activity().unix_secs());
        let pinned = post.flags & PINNED != 0;
        let title: SharedString = post.name.clone().unwrap_or_default().into();
        div()
            .pb_2()
            .child(
                div()
                    .id(("post", ix))
                    .flex()
                    .gap_3()
                    .p_3()
                    .rounded(theme::radius::card())
                    .border_1()
                    .border_color(theme::border())
                    .bg(rgb(theme::base_low()))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme::surface_high())))
                    .on_click(cx.listener(move |this, _, window, cx| this.select_channel(id, window, cx)))
                    .child(
                        div()
                            .flex_1()
                            .min_w(zoomed(0.))
                            .flex()
                            .flex_col()
                            .gap_1()
                            .when(!tags.is_empty() || pinned, |card| {
                                card.child(
                                    div()
                                        .flex()
                                        .flex_wrap()
                                        .gap_1()
                                        .when(pinned, |row| row.child(tag(icons_pin_label())))
                                        .children(tags.iter().map(|t| {
                                            let label = match &t.emoji_name {
                                                Some(emoji) => format!("{emoji} {}", t.name),
                                                None => t.name.clone(),
                                            };
                                            tag(label)
                                        })),
                                )
                            })
                            .child(
                                div()
                                    .text_size(zoomed(16.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(theme::text_strong()))
                                    .truncate()
                                    .child(title),
                            )
                            .children(preview)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .text_xs()
                                    .text_color(theme::muted())
                                    .child(icon(icons::THREAD, 14., theme::text_muted()))
                                    .child(count.to_string())
                                    .child("·")
                                    .child(activity),
                            ),
                    )
                    .children(thumbnail),
            )
            .into_any_element()
    }
}

fn icons_pin_label() -> String {
    format!("📌 {}", t!("forum.pinned"))
}

fn tag(label: impl Into<SharedString>) -> gpui::Div {
    div()
        .px_2()
        .rounded_full()
        .bg(rgb(theme::base_lower()))
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(theme::text_default()))
        .child(label.into())
}
