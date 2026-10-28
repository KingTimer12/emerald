//! What the full profile shows between the user and someone else: the
//! note, and the servers and friends they share.

use super::*;

use gpui::Focusable;

use emerald_discord::Command;
use crate::zoom::zoomed;

/// The tabs of the full profile.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in super::super) enum ProfileTab {
    About,
    Servers,
    Friends,
}

impl Emerald {
    /// Shows `user`'s note in the note field, asking Discord for it the
    /// first time.
    pub(in super::super) fn load_note(&mut self, user: Snowflake, cx: &mut Context<Self>) {
        self.note_user = Some(user);
        match self.people.note(user) {
            Some(note) => {
                let note = note.to_owned();
                self.note_input.update(cx, |input, cx| input.set_text(note, cx));
            }
            None => {
                self.note_input.update(cx, |input, cx| input.set_text("", cx));
                self.net.post(Command::FetchNote { user });
            }
        }
    }

    /// USER_NOTE_UPDATE or a note read: shown at once when its profile is
    /// open, unless the user is typing in it.
    pub(in super::super) fn on_note(&mut self, user: Snowflake, note: String, window: &Window, cx: &mut Context<Self>) {
        let typing = self.note_input.read(cx).focus_handle(cx).is_focused(window);
        if self.note_user == Some(user) && !typing {
            let text = note.clone();
            self.note_input.update(cx, |input, cx| input.set_text(text, cx));
        }
        self.people.set_note(user, note);
    }

    /// Enter in the note field.
    pub(in super::super) fn save_note(&mut self, note: String, cx: &mut Context<Self>) {
        let Some(user) = self.note_user else { return };
        let note = note.trim().to_owned();
        if self.people.note(user) == Some(note.as_str()) {
            return;
        }
        self.people.set_note(user, note.clone());
        self.net.post(Command::SetNote { user, note });
        cx.notify();
    }

    pub(in super::super) fn set_profile_tab(&mut self, user: Snowflake, tab: ProfileTab, cx: &mut Context<Self>) {
        self.profile_tab = tab;
        if tab == ProfileTab::Friends && !self.mutual_friends.contains_key(&user) {
            self.net.post(Command::MutualFriends { user });
        }
        cx.notify();
    }

    /// The "Note" section: a field saved on Enter.
    pub(in super::super) fn note_section(&self) -> Div {
        section(
            t!("note.title"),
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(div().px_2().rounded_md().bg(rgb(theme::base_lower())).child(self.note_input.clone()))
                .child(div().text_xs().text_color(theme::muted()).child(t!("note.hint"))),
        )
    }

    /// The tab strip under the name, with the counts Discord gives.
    pub(in super::super) fn profile_tabs(&self, user: Snowflake, guild: Option<Snowflake>, cx: &mut Context<Self>) -> Div {
        let profile = self.profiles.get(&(user, guild));
        let servers = profile.and_then(|p| p.mutual_guilds.as_ref()).map(Vec::len);
        let friends = self
            .mutual_friends
            .get(&user)
            .map(Vec::len)
            .or(profile.and_then(|p| p.mutual_friends_count).map(|n| n as usize));
        let label = |text: &'static str, count: Option<usize>| match count {
            Some(count) => format!("{text} ({count})"),
            None => text.to_owned(),
        };
        let tab = |id: &'static str, text: String, tab: ProfileTab, cx: &mut Context<Self>| {
            let on = self.profile_tab == tab;
            div()
                .id(id)
                .pb_2()
                .border_b_2()
                .border_color(if on { theme::text() } else { gpui::transparent_black() })
                .text_size(zoomed(14.))
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .text_color(rgb(if on { theme::text_strong() } else { theme::interactive() }))
                .hover(|s| s.text_color(rgb(theme::text_strong())))
                .child(text)
                .on_click(cx.listener(move |this, _, _, cx| this.set_profile_tab(user, tab, cx)))
        };
        div()
            .flex()
            .gap_5()
            .border_b_1()
            .border_color(theme::border())
            .child(tab("profile-about", t!("profile.about_tab").to_owned(), ProfileTab::About, cx))
            .child(tab("profile-servers", label(t!("profile.mutual_servers"), servers), ProfileTab::Servers, cx))
            .child(tab("profile-friends", label(t!("profile.mutual_friends"), friends), ProfileTab::Friends, cx))
    }

    /// The servers both are in, with their nickname there; a click opens one.
    pub(in super::super) fn mutual_servers(&self, user: Snowflake, guild: Option<Snowflake>, cx: &mut Context<Self>) -> Div {
        let shared = self.profiles.get(&(user, guild)).and_then(|p| p.mutual_guilds.clone());
        let Some(shared) = shared else { return empty(t!("common.loading")) };
        if shared.is_empty() {
            return empty(t!("profile.no_mutual_servers"));
        }
        div().flex().flex_col().children(shared.into_iter().enumerate().filter_map(|(ix, mutual)| {
            let space = self.spaces.iter().find(|s| s.id == mutual.id)?;
            let icon = space.icon_url.as_ref().and_then(|url| self.avatar_image(url));
            let id = mutual.id;
            Some(
                row(("mutual-server", ix))
                    .child(
                        div()
                            .size(zoomed(40.))
                            .flex_none()
                            .rounded_xl()
                            .overflow_hidden()
                            .bg(theme::hover())
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(zoomed(14.))
                            .text_color(rgb(theme::text_strong()))
                            .map(|tile| match icon {
                                Some(image) => tile.child(img(image).size_full()),
                                None => tile.child(space.initials.clone()),
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(zoomed(0.))
                            .flex()
                            .flex_col()
                            .child(div().truncate().font_weight(FontWeight::MEDIUM).text_color(rgb(theme::text_strong())).child(space.name.clone()))
                            .children(mutual.nick.map(|nick| div().text_xs().text_color(theme::muted()).child(nick))),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.close_overlay(window, cx);
                        if let Some(ix) = this.spaces.index(id) {
                            this.select_space(ix, window, cx);
                        }
                    })),
            )
        }))
    }

    /// The friends both have; a click opens theirs.
    pub(in super::super) fn mutual_friends_list(&self, user: Snowflake, cx: &mut Context<Self>) -> Div {
        let Some(friends) = self.mutual_friends.get(&user) else { return empty(t!("common.loading")) };
        if friends.is_empty() {
            return empty(t!("profile.no_mutual_friends"));
        }
        div().flex().flex_col().children(friends.iter().enumerate().map(|(ix, friend)| {
            let friend = *friend;
            let name: SharedString = self.people.friend_name(friend).unwrap_or("?").to_owned().into();
            row(("mutual-friend", ix))
                .child(widgets::avatar(&name, friend, self.user_avatar(friend), 40., Some((self.people.presence(friend), theme::base_lowest()))))
                .child(div().flex_1().min_w(zoomed(0.)).truncate().font_weight(FontWeight::MEDIUM).text_color(rgb(theme::text_strong())).child(name))
                .on_click(cx.listener(move |this, _, window, cx| this.open_profile(friend, None, window, cx)))
        }))
    }
}

fn row(id: (&'static str, usize)) -> gpui::Stateful<Div> {
    div()
        .id(id)
        .h(zoomed(56.))
        .flex_none()
        .flex()
        .items_center()
        .gap_3()
        .px_2()
        .rounded_md()
        .cursor_pointer()
        .hover(|s| s.bg(theme::hover()))
}

fn empty(text: &'static str) -> Div {
    div().py_6().flex().justify_center().text_color(theme::muted()).child(text)
}
