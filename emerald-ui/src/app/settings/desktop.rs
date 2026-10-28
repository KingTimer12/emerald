//! Desktop App: opening on login, and the version with its updates.

use super::*;
use super::super::Update;

impl Emerald {
    pub(super) fn render_desktop(&self, cx: &mut Context<Self>) -> AnyElement {
        let login = self.opens_on_login;
        let updates = self.desktop.updates();
        let status = match &self.update {
            _ if !updates => Some(SharedString::from(t!("desktop.packaged_only"))),
            Update::Idle => None,
            Update::Checking => Some(t!("desktop.checking").into()),
            Update::UpToDate => Some(t!("desktop.up_to_date").into()),
            Update::Downloading(version) => Some(t!("desktop.downloading", version = version).into()),
            Update::Ready(version) => Some(t!("desktop.ready", version = version).into()),
            Update::Failed => Some(t!("desktop.failed").into()),
        };
        let busy = matches!(self.update, Update::Checking | Update::Downloading(_));
        let button = if matches!(self.update, Update::Ready(_)) {
            widgets::button("restart-to-update", t!("desktop.restart"), ButtonKind::Positive)
                .on_click(cx.listener(|this, _, window, cx| this.restart_to_update(window, cx)))
        } else {
            let enabled = updates && !busy;
            widgets::button("check-for-updates", t!("desktop.check"), ButtonKind::Secondary)
                .when(!enabled, |button| button.opacity(0.5).cursor_default())
                .on_click(cx.listener(move |this, _, _, cx| {
                    if enabled {
                        this.desktop.check_for_updates();
                        this.update = Update::Checking;
                        cx.notify();
                    }
                }))
        };

        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(page_title(t!("settings.desktop")))
            .child(section_title(t!("desktop.startup")))
            .child(
                switch_row(
                    "open-on-login",
                    t!("desktop.open_on_login"),
                    if login.is_some() { t!("desktop.open_on_login_hint") } else { t!("desktop.packaged_only") },
                    login == Some(true),
                    login.is_some(),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(on) = login {
                        this.set_opens_on_login(!on, cx);
                    }
                })),
            )
            .child(section_title(t!("desktop.updates")).mt_6())
            .child(hint(t!("desktop.version", version = env!("CARGO_PKG_VERSION"))))
            .children(status.map(hint))
            .child(div().mt_2().flex().child(button))
            .into_any_element()
    }
}
