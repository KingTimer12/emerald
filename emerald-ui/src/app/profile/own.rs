//! The popover over the user panel, opened by a click on the user's own
//! avatar or name, as in Discord 2026: their profile card, then Edit
//! Profile, the status, switching accounts and copying their id. Status and
//! accounts open a second panel beside it while hovered.

use std::time::Instant;

use gpui::{AnyElement, ClipboardItem, Stateful};

use super::*;
use super::super::account::now_ms;
use super::super::settings::Page;
use super::super::widgets::popover::{self, Anchor, Tracked};
use crate::icons;
use emerald_discord::model::Status;
use emerald_discord::token::Accounts;

const WIDTH: f32 = 300.;
const SUB_WIDTH: f32 = 240.;
const BANNER: f32 = 105.;
const AVATAR: f32 = 80.;
const RING: f32 = 6.;

/// The second panel, beside the item that opened it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Sub {
    Status,
    Accounts,
}

pub(in super::super) struct OwnPopover {
    anchor: Anchor,
    sub: Option<Sub>,
    /// The popover, the items opening a panel and that panel, as last
    /// drawn, to line the panel up with its item and keep presses on it.
    panel: Tracked,
    sub_panel: Tracked,
    status_item: Tracked,
    accounts_item: Tracked,
    /// The accounts signed in here, read when the popover opens.
    accounts: Accounts,
}

impl Emerald {
    /// Opens the popover at `anchor`, or closes it when it is open.
    pub(in super::super) fn toggle_own_popover(&mut self, anchor: Anchor, cx: &mut Context<Self>) {
        let closed = self.own_popover_dismissed.take();
        if self.own_popover.take().is_none() && !popover::just_closed(closed, Instant::now()) {
            let me = self.people.my_id();
            self.profile(me, None, false);
            self.profile_opened = Instant::now();
            self.own_popover = Some(OwnPopover {
                anchor,
                sub: None,
                panel: Tracked::default(),
                sub_panel: Tracked::default(),
                status_item: Tracked::default(),
                accounts_item: Tracked::default(),
                accounts: Accounts::load(),
            });
        }
        cx.notify();
    }

    fn close_own_popover(&mut self, cx: &mut Context<Self>) {
        if self.own_popover.take().is_some() {
            cx.notify();
        }
    }

    /// Hovering an item opens its panel, or closes the one open.
    fn hover_sub(&mut self, sub: Option<Sub>, cx: &mut Context<Self>) {
        if let Some(own) = &mut self.own_popover
            && own.sub != sub
        {
            own.sub = sub;
            cx.notify();
        }
    }

    pub(in super::super) fn render_own_popover(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let own = self.own_popover.as_ref()?;
        let me = self.people.my_id();
        let card = self.card(me, None);
        let status = self.settings.status.unwrap_or(Status::Online);
        let custom = self.account.custom_status(now_ms()).map(|c| c.line()).filter(|line| !line.is_empty());

        let pronouns = card.pronouns.clone();
        let names = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_1()
            .text_size(zoomed(14.))
            .text_color(rgb(theme::text_default()))
            .child(card.username.clone())
            .children(pronouns.map(|pronouns| format!("\u{2022} {pronouns}")))
            .when(!card.badges.is_empty(), |line| {
                line.child(
                    div()
                        .ml_1()
                        .px_1()
                        .py(zoomed(2.))
                        .flex()
                        .gap_1()
                        .rounded(theme::radius::tag())
                        .border_1()
                        .border_color(theme::border_normal())
                        .children(card.badges.iter().enumerate().map(|(ix, (description, url))| {
                            let badge = div()
                                .id(("own-badge", ix))
                                .size(zoomed(18.))
                                .tooltip(widgets::tooltip(description.clone()));
                            match self.avatar_image(url) {
                                Some(image) => badge.child(img(image).size_full()),
                                None => badge,
                            }
                        })),
                )
            });

        let status_item = item(("own-item", 1), icon_dot(status), widgets::status_label(Some(status.as_str())), true)
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if *hovered {
                    this.hover_sub(Some(Sub::Status), cx)
                }
            }));
        let accounts_item = item(("own-item", 2), glyph(icons::PROFILE), t!("own.switch_accounts"), true)
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if *hovered {
                    this.hover_sub(Some(Sub::Accounts), cx)
                }
            }));
        let (status_item, accounts_item) = (own.status_item.track(status_item), own.accounts_item.track(accounts_item));

        let main = own.panel.track(div())
            .id("own-popover")
            .relative()
            .w(zoomed(WIDTH))
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(theme::radius::modal())
            .border_1()
            .border_color(theme::border_normal())
            .bg(rgb(theme::surface_high()))
            .shadow_lg()
            .child(self.banner(&card, BANNER))
            .child(
                div()
                    .absolute()
                    .top(zoomed(BANNER - AVATAR / 2. - RING))
                    .left_4()
                    .child(self.avatar_ring(me, &card, AVATAR, RING)),
            )
            .children(custom.map(|custom| {
                // The custom status, as a bubble beside the avatar.
                div()
                    .absolute()
                    .top(zoomed(BANNER + 2.))
                    .left(zoomed(16. + AVATAR + 2. * RING + 8.))
                    .max_w(zoomed(WIDTH - AVATAR - 2. * RING - 40.))
                    .px_3()
                    .py_1()
                    .rounded_full()
                    .border_1()
                    .border_color(theme::border_normal())
                    .bg(rgb(theme::surface_high()))
                    .shadow_sm()
                    .text_size(zoomed(14.))
                    .text_color(rgb(theme::text_default()))
                    .truncate()
                    .child(custom)
            }))
            .child(
                div()
                    .px_4()
                    .pt(zoomed(AVATAR / 2. + RING + 12.))
                    .pb_3()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_size(zoomed(20.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(theme::text_strong()))
                            .child(card.name.clone()),
                    )
                    .child(names)
                    .children(card.bio.clone().map(|bio| {
                        div().mt_2().text_size(zoomed(14.)).text_color(rgb(theme::text_default())).child(bio)
                    })),
            )
            .child(
                group()
                    .child(
                        item(("own-item", 0), glyph(icons::EDIT), t!("own.edit_profile"), false)
                            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                                if *hovered {
                                    this.hover_sub(None, cx)
                                }
                            }))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.own_popover = None;
                                this.open_settings(Page::Profiles, window, cx);
                            })),
                    )
                    .child(separator())
                    .child(status_item),
            )
            .child(
                group()
                    .mb_3()
                    .child(accounts_item)
                    .child(separator())
                    .child(
                        item(("own-item", 3), glyph(icons::COPY), t!("own.copy_id"), false)
                            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                                if *hovered {
                                    this.hover_sub(None, cx)
                                }
                            }))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(me.to_string()));
                                this.close_own_popover(cx);
                            })),
                    ),
            )
            .children(self.effect(&card, WIDTH));

        // The second panel's top meets its item's. It is placed out of the
        // layout, so opening it never moves the popover and the item with it.
        let sub = own.sub.map(|sub| {
            let item = if sub == Sub::Status { &own.status_item } else { &own.accounts_item };
            let offset = item.bounds().top() - own.panel.bounds().top();
            let panel = own
                .sub_panel
                .track(popover::panel("own-sub", SUB_WIDTH))
                .absolute()
                .left(zoomed(WIDTH + 8.))
                .top(offset.max(gpui::Pixels::ZERO));
            match sub {
                Sub::Status => self.status_panel(panel, status, custom_set(self), cx),
                Sub::Accounts => self.accounts_panel(panel, &own.accounts, cx),
            }
        });
        let both = div()
            .id("own-popovers")
            .relative()
            .occlude()
            .on_mouse_down_out(cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                // The side panel lies outside this box; presses there are its own.
                if let Some(own) = &this.own_popover
                    && own.sub.is_some()
                    && own.sub_panel.bounds().contains(&event.position)
                {
                    return;
                }
                this.own_popover_dismissed = this.own_popover.take().map(|_| Instant::now());
                cx.notify();
            }))
            .child(main)
            .children(sub);
        Some(popover::at(own.anchor, both))
    }

    /// The four statuses, then the custom status.
    fn status_panel(&self, panel: Stateful<Div>, current: Status, custom: bool, cx: &mut Context<Self>) -> AnyElement {
        panel
            .children(Status::ALL.into_iter().map(|status| {
                let hint = match status {
                    Status::Dnd => Some(t!("settings.dnd_hint")),
                    Status::Invisible => Some(t!("settings.invisible_hint")),
                    _ => None,
                };
                div()
                    .id(("own-status", status as usize))
                    .px_2()
                    .py(zoomed(6.))
                    .flex()
                    .gap_2()
                    .rounded(theme::radius::control())
                    .cursor_pointer()
                    .hover(|s| s.bg(theme::hover()))
                    .when(status == current, |row| row.bg(theme::selected()))
                    .child(div().mt(zoomed(4.)).child(icon_dot(status)))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .min_w(zoomed(0.))
                            .child(
                                div()
                                    .text_size(zoomed(14.))
                                    .text_color(rgb(theme::text_strong()))
                                    .child(widgets::status_label(Some(status.as_str()))),
                            )
                            .children(hint.map(|hint| {
                                div().text_xs().text_color(rgb(theme::text_muted())).child(hint)
                            })),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.own_popover = None;
                        this.set_status(status, cx);
                        cx.notify();
                    }))
            }))
            .child(popover::separator())
            .child(
                super::super::place_menu::item(
                    ("own-custom", 0),
                    if custom { t!("own.edit_custom_status") } else { t!("own.set_custom_status") }.into(),
                    None,
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.own_popover = None;
                    this.open_settings(Page::Status, window, cx);
                })),
            )
            .into_any_element()
    }

    /// The accounts signed in here; another one restarts Emerald on it.
    fn accounts_panel(&self, panel: Stateful<Div>, accounts: &Accounts, cx: &mut Context<Self>) -> AnyElement {
        let me = self.people.my_id();
        panel
            .children(accounts.list.iter().enumerate().map(|(ix, account)| {
                let id = account.id;
                let name: SharedString = match account.display_name() {
                    "" => id.to_string().into(),
                    name => name.to_owned().into(),
                };
                let avatar = self.avatar_image(&emerald_discord::model::avatar_url(id, account.avatar.as_deref()));
                div()
                    .id(("own-account", ix))
                    .px_2()
                    .py(zoomed(6.))
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded(theme::radius::control())
                    .when(id != me, |row| row.cursor_pointer().hover(|s| s.bg(theme::hover())))
                    .child(widgets::avatar(&name, id, avatar, 24., None))
                    .child(
                        div()
                            .flex_1()
                            .min_w(zoomed(0.))
                            .truncate()
                            .text_size(zoomed(14.))
                            .text_color(rgb(theme::text_strong()))
                            .child(name),
                    )
                    .when(id == me, |row| row.child(icons::icon(icons::CHECK, 16., theme::brand())))
                    .when(id != me, |row| row.on_click(move |_, _, _| emerald_discord::switch_account(id)))
            }))
            .child(popover::separator())
            .child(
                super::super::place_menu::item(("own-manage-accounts", 0), t!("own.manage_accounts").into(), None)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.own_popover = None;
                        this.open_settings(Page::Accounts, window, cx);
                    })),
            )
            .into_any_element()
    }
}

/// Whether a custom status is set, for "Set" or "Edit".
fn custom_set(this: &Emerald) -> bool {
    this.account.custom_status(now_ms()).is_some()
}

/// A bordered group of items, as the popover's two lists.
fn group() -> Div {
    div()
        .mx_3()
        .mt_3()
        .flex()
        .flex_col()
        .px_1()
        .rounded(theme::radius::card())
        .border_1()
        .border_color(theme::border_normal())
        .bg(rgb(theme::base_lower()))
}

fn separator() -> Div {
    div().mx_2().h(zoomed(1.)).bg(theme::border_normal())
}

/// One row of a group: an icon, the label and, when it opens a panel, a
/// chevron.
fn item(id: (&'static str, usize), leading: AnyElement, label: impl Into<SharedString>, opens: bool) -> Stateful<Div> {
    div()
        .id(id)
        .h(zoomed(44.))
        .px_3()
        .flex()
        .items_center()
        .gap_3()
        .rounded(theme::radius::control())
        .cursor_pointer()
        .hover(|s| s.bg(theme::hover()))
        .child(div().w(zoomed(18.)).flex().justify_center().child(leading))
        .child(
            div()
                .flex_1()
                .min_w(zoomed(0.))
                .truncate()
                .text_size(zoomed(14.))
                .text_color(rgb(theme::text_default()))
                .child(label.into()),
        )
        .when(opens, |row| row.child(icons::icon(icons::CHEVRON_RIGHT, 16., theme::interactive())))
}

fn glyph(path: &'static str) -> AnyElement {
    icons::icon(path, 18., theme::interactive()).into_any_element()
}

/// The status's colored dot, as on avatars.
fn icon_dot(status: Status) -> AnyElement {
    div()
        .size(zoomed(12.))
        .rounded_full()
        .bg(rgb(widgets::status_color(status.as_str())))
        .into_any_element()
}
