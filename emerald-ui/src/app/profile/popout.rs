//! The profile card a click on someone's name or avatar opens beside it,
//! as in Discord 2026: banner with the friend and "More" buttons, avatar,
//! name, tag and badges, what is shared, the bio, and a box to message
//! them. Also the theme and badge pieces the other profile views share.

use std::time::Instant;

use gpui::{AnyElement, Entity, Subscription, linear_color_stop, linear_gradient, rgba};

use super::*;
use super::super::state::profile_card::{self, badge_overflow};
use super::super::widgets::popover::{self, Anchor, Tracked};
use crate::icons;
use crate::text_input::{Submitted, TextInput};
use emerald_discord::ProfileKind;
use emerald_discord::model::relationship_kind;

const WIDTH: f32 = 300.;
const BANNER: f32 = 105.;
const AVATAR: f32 = 80.;
const RING: f32 = 6.;
/// How long a message typed in a card waits for its DM to open; past it,
/// or once opening failed, it is dropped rather than sent by surprise.
const POPOUT_SEND_WINDOW: std::time::Duration = std::time::Duration::from_secs(30);

pub(in super::super) struct Popout {
    user: Snowflake,
    guild: Option<Snowflake>,
    anchor: Anchor,
    message: Entity<TextInput>,
    more: Tracked,
    _subscription: Subscription,
}

impl Emerald {
    /// Opens `user`'s card at `anchor`, or closes it when it is theirs and
    /// open. The user's own card is the popover of the user panel.
    pub(in super::super) fn open_popout(
        &mut self,
        user: Snowflake,
        guild: Option<Snowflake>,
        anchor: Anchor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.people.is_me(user) {
            return self.toggle_own_popover(anchor, cx);
        }
        let closed = self.popout_dismissed.take();
        if self.popout.take().is_some_and(|p| p.user == user) || popover::just_closed(closed, Instant::now()) {
            cx.notify();
            return;
        }
        self.profile(user, guild, ProfileKind::Popout);
        self.profile_opened = Instant::now();
        let name = self.people.user(user).map(|u| u.username.clone()).unwrap_or_default();
        let message = cx.new(|cx| TextInput::new(t!("profile.message_user", name = name), cx));
        let subscription = cx.subscribe_in(&message, window, move |this, _, event: &Submitted, window, cx| {
            this.message_from_popout(user, event.0.clone(), window, cx)
        });
        self.popout = Some(Popout {
            user,
            guild,
            anchor,
            message,
            more: Tracked::default(),
            _subscription: subscription,
        });
        cx.notify();
    }

    pub(in super::super) fn close_popout(&mut self, cx: &mut Context<Self>) {
        if self.popout.take().is_some() {
            cx.notify();
        }
    }

    /// Sends `text` to `user` in their DM, opening it first when there is
    /// none yet, as the web client's box does; then goes there.
    fn message_from_popout(&mut self, user: Snowflake, text: String, window: &mut Window, cx: &mut Context<Self>) {
        let text = text.trim().to_owned();
        if text.is_empty() {
            return;
        }
        self.close_popout(cx);
        match self.spaces.dm_with(user) {
            Some(channel) => {
                self.net.post(Command::Send {
                    channel,
                    content: text,
                    nonce: emerald_discord::rest::nonce(),
                    reply: None,
                    files: Vec::new(),
                });
                self.spaces.select_dms();
                self.select_channel(channel, window, cx);
            }
            None => {
                self.popout_pending = Some((user, text, Instant::now()));
                self.net.post(Command::OpenDm { user, select: true });
            }
        }
    }

    /// A DM opened: sends the message the popout was waiting to send there.
    pub(in super::super) fn send_popout_pending(&mut self, channel: Snowflake, recipient: Option<Snowflake>) {
        let pending = self.popout_pending.take();
        let Some((_, text, _)) = pending.filter(|(user, _, at)| Some(*user) == recipient && at.elapsed() < POPOUT_SEND_WINDOW)
        else {
            return;
        };
        self.net.post(Command::Send {
            channel,
            content: text,
            nonce: emerald_discord::rest::nonce(),
            reply: None,
            files: Vec::new(),
        });
    }

    pub(in super::super) fn render_popout(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let popout = self.popout.as_ref()?;
        let (user, guild) = (popout.user, popout.guild);
        let card = self.card(user, guild);
        let text_color = card.theme.map_or(theme::text_strong(), |t| t.text);
        let kind = self.people.relationship(user);
        let bot = self.people.user(user).is_some_and(|u| u.bot);
        let (friend_icon, friend_tip) = match kind {
            Some(relationship_kind::FRIEND) => (icons::FRIENDS, t!("profile.friend")),
            Some(relationship_kind::OUTGOING) => (icons::USER_ADD, t!("profile.outgoing")),
            Some(relationship_kind::INCOMING) => (icons::USER_ADD, t!("profile.incoming")),
            _ => (icons::USER_ADD, t!("profile.add_friend")),
        };
        let more = popout.more.clone();
        let buttons = div()
            .absolute()
            .top_2()
            .right_2()
            .flex()
            .gap_1()
            .when(!bot && kind != Some(relationship_kind::BLOCKED), |row| {
                row.child(round_button("popout-friend", friend_icon, friend_tip).on_click(cx.listener(
                    move |this, _, _, cx| {
                        if this.people.relationship(user).is_none() {
                            // The same PUT as accepting a request; the web
                            // client's location for the popout is a guess.
                            this.net.post(Command::AcceptFriend { user });
                            cx.notify();
                        } else {
                            this.open_user_menu(user, guild, None, Anchor::pointer(Default::default()), cx);
                        }
                    },
                )))
            })
            .child(more.track(round_button("popout-more", icons::MORE, t!("profile.more"))).on_click(cx.listener(
                move |this, _, _, cx| this.open_user_menu(user, guild, None, Anchor::below_end(more.bounds()), cx),
            )));
        let body = div()
            .relative()
            .px_4()
            .pt(zoomed(AVATAR / 2. + RING + 12.))
            .pb_4()
            .flex()
            .flex_col()
            .gap_2()
            .text_color(rgb(text_color))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        widgets::name::styled_name(card.name.clone(), card.name_style.as_ref(), text_color, true, super::super::state::profile_theme::card_background(card.theme.as_ref()), cx)
                            .text_size(zoomed(20.))
                            .font_weight(FontWeight::BOLD),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_1()
                            .text_size(zoomed(14.))
                            .child(card.username.clone())
                            .children(card.tag.as_ref().map(|tag| {
                                let badge = tag.badge_url(28).and_then(|url| self.avatar_image(&url));
                                widgets::name::tag_chip("popout-tag", tag.tag.as_deref().unwrap_or_default(), badge, 14.)
                            }))
                            .children(self.badges_pill("popout-badge", &card)),
                    ),
            )
            .children(self.mutual_row(&card, user, guild, cx))
            .children(card.bio.clone().map(|bio| div().text_size(zoomed(14.)).line_clamp(6).child(bio)))
            .when(!bot && kind != Some(relationship_kind::BLOCKED), |body| {
                body.child(
                    div()
                        .mt_1()
                        .px_3()
                        .py_2()
                        .rounded(theme::radius::control())
                        .border_1()
                        .border_color(theme::border_normal())
                        .bg(rgb(card.theme.map_or(theme::base_lowest(), |t| t.base)))
                        .text_size(zoomed(14.))
                        .child(popout.message.clone()),
                )
            });
        let panel = self
            .themed(
                div()
                    .id("user-popout")
                    .relative()
                    .w(zoomed(WIDTH))
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .rounded(theme::radius::modal())
                    .border_1()
                    .border_color(theme::border_normal())
                    .shadow_lg()
                    .occlude()
                    .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                        // A menu the card opened lies outside it; it closes itself.
                        if this.user_menu.is_some() {
                            return;
                        }
                        this.popout_dismissed = this.popout.take().map(|_| Instant::now());
                        cx.notify();
                    })),
                card.theme.as_ref(),
            )
            .child(div().relative().child(self.banner(&card, BANNER)).child(buttons))
            .child(
                div()
                    .absolute()
                    .top(zoomed(BANNER - AVATAR / 2. - RING))
                    .left(zoomed(10.))
                    .child(self.avatar_ring(user, &card, AVATAR, RING)),
            )
            .child(body)
            .children(self.effect(&card, WIDTH));
        Some(popover::at(popout.anchor, panel))
    }

    /// `card` drawn on the profile's theme: a vertical gradient from the
    /// primary to the accent color under a veil that keeps text readable;
    /// the usual surface without theme colors.
    pub(super) fn themed<E: ParentElement + Styled>(&self, card: E, theme: Option<&ProfileTheme>) -> E {
        let Some(theme) = theme else { return card.bg(rgb(theme::surface_high())) };
        let (veil, alpha) = theme.overlay;
        card.bg(linear_gradient(
            180.,
            linear_color_stop(rgb(theme.primary), 0.),
            linear_color_stop(rgb(theme.accent), 1.),
        ))
        .child(div().absolute().top_0().left_0().size_full().bg(rgba(veil << 8 | (alpha * 255.) as u32)))
    }

    /// The badges in a bordered pill, six at most and "+N" for the rest.
    pub(super) fn badges_pill(&self, id: &'static str, card: &Card) -> Option<Div> {
        if card.badges.is_empty() {
            return None;
        }
        let (shown, more) = badge_overflow(card.badges.len());
        Some(
            div()
                .flex_none()
                .px_1()
                .py(zoomed(2.))
                .flex()
                .items_center()
                .gap_1()
                .rounded(theme::radius::tag())
                .border_1()
                .border_color(theme::border_normal())
                .children(card.badges.iter().take(shown).enumerate().map(|(ix, (description, url))| {
                    let badge = div().id((id, ix)).size(zoomed(18.)).tooltip(widgets::tooltip(description.clone()));
                    match self.avatar_image(url) {
                        Some(image) => badge.child(img(image).size_full()),
                        None => badge,
                    }
                }))
                .when(more > 0, |pill| {
                    pill.child(div().text_size(zoomed(12.)).font_weight(FontWeight::SEMIBOLD).child(format!("+{more}")))
                }),
        )
    }

    /// Up to three friends in common, stacked, then how many friends and
    /// servers are shared; a click opens the full profile.
    pub(super) fn mutual_row(&self, card: &Card, user: Snowflake, guild: Option<Snowflake>, cx: &mut Context<Self>) -> Option<AnyElement> {
        let line = profile_card::mutual_line(card.mutual_friend_count.unwrap_or(0), card.mutual_server_count.unwrap_or(0))?;
        let small = card.mutual_friend_count.unwrap_or(0) > 0 && card.mutual_server_count.unwrap_or(0) > 0;
        let faces = card.mutual_friends.iter().take(3).enumerate().map(|(ix, friend)| {
            div()
                .when(ix > 0, |face| face.ml(zoomed(-4.)))
                .rounded_full()
                .border_1()
                .border_color(rgb(theme::surface_high()))
                .child(widgets::avatar(friend.display_name(), friend.id, self.avatar_image(&friend.avatar_url()), 16., None))
        });
        Some(
            div()
                .id("mutual-row")
                .flex()
                .items_center()
                .gap_2()
                .cursor_pointer()
                .text_size(zoomed(if small { 12. } else { 14. }))
                .hover(|s| s.opacity(0.8))
                .child(div().flex().children(faces))
                .child(line)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.close_popout(cx);
                    this.open_profile(user, guild, window, cx);
                }))
                .into_any_element(),
        )
    }
}

/// A 32px round button on the banner.
fn round_button(id: &'static str, glyph: &'static str, tip: &'static str) -> gpui::Stateful<Div> {
    div()
        .id(id)
        .size(zoomed(32.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(rgba(0x0000_0099))
        .cursor_pointer()
        .hover(|s| s.bg(rgba(0x0000_00cc)))
        .tooltip(widgets::tooltip(tip))
        .child(icons::icon(glyph, 18., 0xffffff))
}
