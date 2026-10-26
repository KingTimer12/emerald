//! Accounts: the accounts signed in on this computer, switching between
//! them (Emerald starts again on the other one) and adding another.

use super::*;
use emerald_discord::token::{Account, MAX_ACCOUNTS};

impl Emerald {
    pub(super) fn render_accounts(&self, view: &SettingsView, cx: &mut Context<Self>) -> AnyElement {
        let me = self.people.my_id();
        let accounts = &view.accounts;
        let adding = self.account.adding;
        let full = accounts.list.len() >= MAX_ACCOUNTS;

        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(page_title(t!("settings.accounts")))
            .child(hint(t!("accounts.hint")).mb_4())
            .children(accounts.list.iter().enumerate().map(|(ix, account)| self.account_row(ix, account, account.id == me, cx)))
            .child(
                div().mt_6().flex().items_center().gap_3().child(
                    widgets::button(
                        "add-account",
                        if adding { t!("accounts.adding") } else { t!("accounts.add") },
                        ButtonKind::Brand,
                    )
                    .when(adding || full, |b| b.opacity(0.5))
                    .when(!adding && !full, |b| b.on_click(cx.listener(|this, _, _, cx| this.add_account(cx)))),
                )
                .when(full, |row| row.child(hint(t!("accounts.full", count = MAX_ACCOUNTS)))),
            )
            .into_any_element()
    }

    fn account_row(&self, ix: usize, account: &Account, active: bool, cx: &mut Context<Self>) -> gpui::Div {
        let id = account.id;
        let name: SharedString = match account.display_name() {
            "" => id.to_string().into(),
            name => name.to_owned().into(),
        };
        let avatar = self.avatar_image(&avatar_url(id, account.avatar.as_deref()));

        div()
            .flex()
            .items_center()
            .gap_3()
            .p_3()
            .rounded_lg()
            .bg(rgb(theme::BASE_LOWEST))
            .child(widgets::avatar(&name, id, avatar, 40., None))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(0.))
                    .child(div().truncate().font_weight(FontWeight::SEMIBOLD).text_color(rgb(theme::TEXT_STRONG)).child(name))
                    .when(!account.username.is_empty(), |col| col.child(hint(account.username.clone()).truncate())),
            )
            .when(active, |row| row.child(hint(t!("accounts.active"))))
            .when(!active, |row| {
                row.child(
                    widgets::button(("switch-account", ix), t!("accounts.switch"), ButtonKind::Brand)
                        .on_click(move |_, _, _| emerald_discord::switch_account(id)),
                )
                .child(
                    widgets::button(("remove-account", ix), t!("accounts.remove"), ButtonKind::Secondary).on_click(
                        cx.listener(move |this, _, _, cx| {
                            emerald_discord::token::remove(id);
                            if let Some(view) = this.settings_view() {
                                view.accounts = emerald_discord::token::Accounts::load();
                            }
                            cx.notify();
                        }),
                    ),
                )
            })
    }
}
