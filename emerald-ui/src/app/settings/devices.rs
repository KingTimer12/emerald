//! Devices: where the account is signed in, with this one apart, and
//! logging the others out.

use super::*;
use crate::time;
use emerald_discord::model::{AuthSession, iso_secs};

impl Emerald {
    pub(super) fn render_devices(&self, view: &SettingsView, cx: &mut Context<Self>) -> AnyElement {
        let page = div().flex().flex_col().gap_2().child(page_title(t!("settings.devices"))).child(hint(t!("devices.hint")));
        let sessions = match self.account.sessions() {
            None => return page.child(hint(t!("devices.loading")).mt_4()).into_any_element(),
            Some(Err(error)) => {
                return page
                    .child(widgets::feedback(&Feedback::from_result(Err(error.clone()), "")).mt_4())
                    .into_any_element();
            }
            Some(Ok(_)) => self.account.other_sessions(),
        };
        let busy = self.busy.contains(&Action::Sessions);
        let hashes: Vec<String> = sessions.iter().map(|s| s.id_hash.clone()).collect();

        page.child(section_title(t!("devices.current")).mt_6())
            .children(self.account.this_session().map(|s| device_row(s, None, cx)))
            .child(section_title(t!("devices.others")).mt_6())
            .when(sessions.is_empty(), |page| page.child(hint(t!("devices.no_others"))))
            .children(sessions.iter().enumerate().map(|(ix, s)| device_row(s, Some(ix), cx)))
            .when(!sessions.is_empty(), |page| {
                page.child(
                    div().mt_6().flex().child(
                        widgets::button("log-out-all", t!("devices.log_out_all"), ButtonKind::Danger)
                            .when(busy, |b| b.opacity(0.5))
                            .on_click(cx.listener(move |this, _, _, cx| this.log_out_sessions(hashes.clone(), cx))),
                    ),
                )
            })
            .children(view.feedback.as_ref().map(|f| widgets::feedback(f).mt_4()))
            .into_any_element()
    }
}

/// One device; `ix` is set for the others, which get a log-out button.
fn device_row(session: &AuthSession, ix: Option<usize>, cx: &mut Context<Emerald>) -> gpui::Div {
    let info = &session.client_info;
    let name = [info.os.as_deref(), info.platform.as_deref()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
    let name = if name.is_empty() { t!("devices.unknown").to_owned() } else { name };
    let used = session
        .approx_last_used_time
        .as_deref()
        .and_then(iso_secs)
        .filter(|_| ix.is_some())
        .map(time::short_date);
    let detail = [info.location.clone(), used].into_iter().flatten().collect::<Vec<_>>().join(" · ");
    let hash = session.id_hash.clone();

    div()
        .flex()
        .items_center()
        .gap_3()
        .py_3()
        .border_b_1()
        .border_color(theme::border())
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w(px(0.))
                .child(div().truncate().font_weight(FontWeight::MEDIUM).text_color(rgb(theme::TEXT_STRONG)).child(name))
                .when(!detail.is_empty(), |col| col.child(hint(detail).truncate())),
        )
        .children(ix.map(|ix| {
            widgets::button(("log-out-device", ix), t!("devices.log_out"), ButtonKind::Secondary)
                .on_click(cx.listener(move |this, _, _, cx| this.log_out_sessions(vec![hash.clone()], cx)))
        }))
}
