//! Calls in DMs and group DMs: the phone button in the header, the bar
//! over the messages while a call goes on (join, or decline when it rings
//! for us), and ringing the others when we start one.

use gpui::{AnyElement, Context, FontWeight, SharedString, div, prelude::*, rgb};

use super::super::super::{icons, theme};
use super::super::widgets::{self, ButtonKind};
use super::super::Emerald;
use emerald_discord::Command;
use emerald_discord::model::{CallUpdate, Snowflake};
use crate::zoom::zoomed;

impl Emerald {
    /// Starts a call in `channel`, or joins the one going on.
    pub(in crate::app) fn start_dm_call(&mut self, channel: Snowflake, cx: &mut Context<Self>) {
        if self.call.as_ref().is_some_and(|call| call.channel == channel) {
            return;
        }
        // Ring only when nobody is in it yet; joining a call rings no one.
        let ring = self.dm_calls.get(channel).is_none_or(|call| call.users.is_empty());
        self.start_call(None, channel, ring, cx);
    }

    /// Our phone stops ringing for this call.
    fn decline_call(&mut self, channel: Snowflake, cx: &mut Context<Self>) {
        let me = self.people.my_id();
        self.net.post(Command::StopRinging { channel, recipients: vec![me] });
        cx.notify();
    }

    /// CALL_CREATE and CALL_UPDATE.
    pub(in crate::app) fn on_call(&mut self, update: CallUpdate, cx: &mut Context<Self>) {
        let channel = update.channel_id;
        let me = self.people.my_id();
        // The call we started exists now: ring the others, as the web client does.
        if let Some(call) = self.call.as_mut().filter(|call| call.channel == channel && call.ring_on_create) {
            call.ring_on_create = false;
            self.net.post(Command::RingCall { channel });
        }
        if self.dm_calls.update(update, me) && self.call.as_ref().is_none_or(|call| call.channel != channel) {
            let name = self.spaces.dms().label(channel).cloned().unwrap_or_default();
            self.alerts.notify(&name, t!("voice.incoming"), channel);
        }
        cx.notify();
    }

    pub(in crate::app) fn on_call_delete(&mut self, channel: Snowflake, cx: &mut Context<Self>) {
        self.dm_calls.remove(channel);
        cx.notify();
    }

    /// The phone in a DM's header.
    pub(in crate::app) fn render_call_button(&self, channel: Snowflake, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.call.as_ref().is_some_and(|call| call.channel == channel) {
            return None;
        }
        Some(
            widgets::svg_button("start-call", icons::PHONE)
                .tooltip(widgets::tooltip(t!("voice.start_call")))
                .on_click(cx.listener(move |this, _, _, cx| this.start_dm_call(channel, cx)))
                .into_any_element(),
        )
    }

    /// Over the messages of a DM while a call goes on there without us.
    pub(in crate::app) fn render_dm_call_bar(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let channel = self.selected_channel?;
        if self.call.as_ref().is_some_and(|call| call.channel == channel) {
            return None;
        }
        let call = self.dm_calls.get(channel)?;
        let me = self.people.my_id();
        let ringing = call.ringing.contains(&me);
        let text: SharedString = if ringing {
            t!("voice.incoming").into()
        } else {
            t!("voice.in_call", count = call.users.len()).into()
        };
        Some(
            div()
                .absolute()
                .top_2()
                .left_4()
                .right_4()
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .py_2()
                .rounded(theme::radius::control())
                .border_1()
                .border_color(theme::border())
                .bg(rgb(theme::base_low()))
                .child(icons::icon(icons::PHONE, 18., theme::online()))
                .child(
                    div()
                        .flex_1()
                        .min_w(zoomed(0.))
                        .truncate()
                        .text_size(zoomed(14.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(theme::text_strong()))
                        .child(text),
                )
                .child(
                    widgets::button("call-join", t!("voice.join"), ButtonKind::Positive)
                        .on_click(cx.listener(move |this, _, _, cx| this.start_dm_call(channel, cx))),
                )
                .when(ringing, |bar| {
                    bar.child(
                        widgets::button("call-decline", t!("voice.decline"), ButtonKind::Danger)
                            .on_click(cx.listener(move |this, _, _, cx| this.decline_call(channel, cx))),
                    )
                })
                .into_any_element(),
        )
    }
}
