//! Message requests: DMs from people the user has not accepted, and those
//! Discord put in the spam inbox. Listed apart from the DMs until accepted;
//! opening one shows its messages with the answer in place of the box.

use gpui::{AnyElement, Context, FontWeight, SharedString, div, prelude::*, rgb};

use super::super::theme;
use super::widgets::{self, ButtonKind};
use super::Emerald;
use emerald_discord::Command;
use emerald_discord::model::{Channel, Snowflake};
use crate::zoom::zoomed;

impl Emerald {
    pub(super) fn open_requests(&mut self, cx: &mut Context<Self>) {
        self.friends_open = false;
        self.requests_open = true;
        cx.notify();
    }

    /// Accepts or ignores a request. An accepted one joins the DMs at once,
    /// as the web client shows it; an ignored one leaves when Discord says
    /// so, and closes if open.
    pub(super) fn answer_request(&mut self, channel: Snowflake, accept: bool, cx: &mut Context<Self>) {
        self.net.post(Command::AnswerRequest { channel, accept });
        let dms = self.spaces.dms_mut();
        if accept && let Some(found) = dms.channel_mut(channel) {
            found.is_message_request = false;
            found.is_spam = false;
            dms.refresh();
        }
        if !accept && self.selected_channel == Some(channel) {
            self.selected_channel = None;
            self.requests_open = true;
        }
        cx.notify();
    }

    fn request_name(&self, channel: &Channel) -> SharedString {
        self.spaces.dms().label(channel.id).cloned().unwrap_or_else(|| self.people.dm_name(channel).into())
    }

    pub(super) fn render_requests(&self, cx: &mut Context<Self>) -> AnyElement {
        let spam = self.requests_spam;
        let dms = self.spaces.dms();
        let mut requests: Vec<&Channel> = dms.requests(spam).collect();
        requests.sort_by_key(|c| std::cmp::Reverse(c.activity()));
        let counts = [dms.requests(false).count(), dms.requests(true).count()];
        let tab = |id: &'static str, label: &'static str, count: usize, on: bool, cx: &mut Context<Self>| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .py(zoomed(2.))
                .rounded(theme::radius::control())
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .text_color(rgb(if on { theme::text_strong() } else { theme::interactive() }))
                .when(on, |t| t.bg(theme::selected()))
                .when(!on, |t| t.hover(|s| s.bg(theme::hover())))
                .child(label)
                .when(count > 0, |t| t.child(widgets::badge(count)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.requests_spam = id == "requests-spam";
                    cx.notify();
                }))
        };
        let header = div()
            .h(zoomed(48.))
            .flex_none()
            .flex()
            .items_center()
            .gap_4()
            .px_4()
            .border_b_1()
            .border_color(theme::border())
            .child(div().text_color(rgb(theme::text_strong())).font_weight(FontWeight::SEMIBOLD).child(t!("requests.title")))
            .child(div().w(zoomed(1.)).h(zoomed(24.)).bg(theme::selected()))
            .child(tab("requests-pending", t!("requests.tab_requests"), counts[0], !spam, cx))
            .child(tab("requests-spam", t!("requests.tab_spam"), counts[1], spam, cx));
        let rows: Vec<AnyElement> = requests.iter().enumerate().map(|(ix, channel)| self.render_request(ix, channel, cx)).collect();
        let body = if rows.is_empty() {
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .px_8()
                .text_color(theme::muted())
                .child(if spam { t!("requests.empty_spam") } else { t!("requests.empty") })
                .into_any_element()
        } else {
            div()
                .id("request-list")
                .flex_1()
                .flex()
                .flex_col()
                .overflow_y_scroll()
                .pl(zoomed(30.))
                .pr(zoomed(20.))
                .pt_4()
                .child(div().text_xs().text_color(theme::muted()).mb_2().child(if spam {
                    t!("requests.spam_hint")
                } else {
                    t!("requests.hint")
                }))
                .children(rows)
                .into_any_element()
        };
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(zoomed(0.))
            .h_full()
            .bg(rgb(theme::base_lower()))
            .border_l_1()
            .border_color(theme::border())
            .child(header)
            .child(body)
            .into_any_element()
    }

    fn render_request(&self, ix: usize, channel: &Channel, cx: &mut Context<Self>) -> AnyElement {
        let id = channel.id;
        let name = self.request_name(channel);
        let user = channel.recipient();
        let spam = channel.is_spam;
        let small = |button: gpui::Stateful<gpui::Div>| button.h(zoomed(32.)).px_3();
        div()
            .id(("request", ix))
            .h(zoomed(62.))
            .flex_none()
            .flex()
            .items_center()
            .gap_3()
            .px_2()
            .border_t_1()
            .border_color(theme::border())
            .rounded(theme::radius::control())
            .cursor_pointer()
            .hover(|s| s.bg(theme::hover()))
            .children(user.map(|user| widgets::avatar(&name, user, self.user_avatar(user), 32., None)))
            .child(
                div()
                    .flex_1()
                    .min_w(zoomed(0.))
                    .truncate()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::text_strong()))
                    .child(name),
            )
            .child(
                small(widgets::button(
                    ("request-ignore", ix),
                    if spam { t!("requests.delete") } else { t!("requests.ignore") },
                    ButtonKind::Secondary,
                ))
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.answer_request(id, false, cx)
                })),
            )
            .child(
                small(widgets::button(
                    ("request-accept", ix),
                    if spam { t!("requests.not_spam") } else { t!("requests.accept") },
                    ButtonKind::Brand,
                ))
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.answer_request(id, true, cx)
                })),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                this.requests_open = false;
                this.select_channel(id, window, cx)
            }))
            .into_any_element()
    }

    /// Under an open request, in place of the message box: who wrote and
    /// the answer, as the web client asks before letting the user reply.
    pub(super) fn render_request_bar(&self, channel: Snowflake, cx: &mut Context<Self>) -> Option<AnyElement> {
        let found = self.spaces.dms().channels().iter().find(|c| c.id == channel && c.is_message_request)?;
        let spam = found.is_spam;
        let name = self.request_name(found);
        Some(
            div()
                .m_4()
                .p_4()
                .flex()
                .flex_col()
                .gap_3()
                .rounded(theme::radius::card())
                .bg(rgb(theme::base_lowest()))
                .border_1()
                .border_color(theme::border())
                .child(div().text_color(rgb(theme::text_default())).child(if spam {
                    t!("requests.spam_bar", name = name)
                } else {
                    t!("requests.bar", name = name)
                }))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            widgets::button("bar-ignore", if spam { t!("requests.delete") } else { t!("requests.ignore") }, ButtonKind::Secondary)
                                .on_click(cx.listener(move |this, _, _, cx| this.answer_request(channel, false, cx))),
                        )
                        .child(
                            widgets::button("bar-accept", if spam { t!("requests.not_spam") } else { t!("requests.accept") }, ButtonKind::Brand)
                                .on_click(cx.listener(move |this, _, _, cx| this.answer_request(channel, true, cx))),
                        ),
                )
                .into_any_element(),
        )
    }
}
