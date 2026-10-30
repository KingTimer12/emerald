//! My Account: display name, email and status.

use super::*;
use crate::zoom::zoomed;

impl Emerald {
    /// An empty name resets the display name to the username.
    pub(super) fn save_display_name(&mut self, name: String, cx: &mut Context<Self>) {
        let name = name.trim();
        let name = (!name.is_empty()).then(|| name.to_owned());
        if let Some(view) = self.settings_view() {
            view.feedback = None;
        }
        self.run(Action::UpdateProfile, Command::SetGlobalName { name }, cx);
    }
}

impl Emerald {
    pub(super) fn render_account(&self, view: &SettingsView, cx: &mut Context<Self>) -> AnyElement {
        let Some(me) = self.people.me() else {
            return page_title(t!("settings.my_account")).into_any_element();
        };
        let name: SharedString = me.display_name().to_owned().into();
        let email: SharedString = match (&me.email, view.reveal_email) {
            (None, _) => t!("settings.no_email").into(),
            (Some(email), true) => email.clone().into(),
            (Some(email), false) => mask_email(email).into(),
        };
        let field = |caption: &'static str, value: SharedString| {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(widgets::label(caption))
                .child(div().text_color(rgb(theme::text_default())).child(value))
        };
        let busy = self.busy.contains(&Action::UpdateProfile);

        div()
            .flex()
            .flex_col()
            .child(page_title(t!("settings.my_account")))
            .child(
                div()
                    .rounded(theme::radius::card())
                    .overflow_hidden()
                    .bg(rgb(theme::base_lowest()))
                    .child(div().h(zoomed(100.)).bg(rgb(theme::brand())))
                    .child(
                        div()
                            .flex()
                            .items_end()
                            .gap_4()
                            .px_4()
                            .child(
                                // Ring around the avatar where it overlaps the banner.
                                div()
                                    .mt(zoomed(-40.))
                                    .p(zoomed(6.))
                                    .rounded_full()
                                    .bg(rgb(theme::base_lowest()))
                                    .child(widgets::avatar(
                                        &name,
                                        me.id,
                                        self.avatar_image(&me.avatar_url()),
                                        80.,
                                        self.own_status().map(|s| (s, theme::base_lowest())),
                                    )),
                            )
                            .child(
                                div()
                                    .pb_3()
                                    .text_size(zoomed(20.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(theme::text_strong()))
                                    .child(name.clone()),
                            ),
                    )
                    .child(
                        div()
                            .m_4()
                            .p_4()
                            .rounded(theme::radius::card())
                            .bg(rgb(theme::base_low()))
                            .flex()
                            .flex_col()
                            .gap_6()
                            .child(field(t!("settings.display_name_label"), name))
                            .child(field(t!("settings.username_label"), me.username.clone().into()))
                            .child(
                                div()
                                    .flex()
                                    .items_end()
                                    .gap_1()
                                    .child(field(t!("settings.email_label"), email))
                                    .when(me.email.is_some(), |row| {
                                        row.child(
                                            div()
                                                .id("reveal-email")
                                                .text_size(zoomed(14.))
                                                .text_color(rgb(theme::link()))
                                                .cursor_pointer()
                                                .hover(|s| s.underline())
                                                .child(if view.reveal_email { t!("common.hide") } else { t!("common.show") })
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    if let Some(view) = this.settings_view() {
                                                        view.reveal_email = !view.reveal_email;
                                                    }
                                                    cx.notify();
                                                })),
                                        )
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .mt_10()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(section_title(t!("settings.edit_display_name")))
                    .child(
                        div()
                            .text_size(zoomed(14.))
                            .text_color(rgb(theme::text_muted()))
                            .child(t!("settings.display_name_hint")),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(div().flex_1().child(view.display_name.clone()))
                            .child(
                                widgets::button(
                                    "save-display-name",
                                    if busy { t!("common.saving") } else { t!("common.save") },
                                    ButtonKind::Brand,
                                )
                                .when(busy, |b| b.opacity(0.5))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let Some(view) = this.settings_view() else { return };
                                    let name = view.display_name.read(cx).text().to_owned();
                                    this.save_display_name(name, cx);
                                })),
                            ),
                    )
                    .children(view.feedback.as_ref().map(widgets::feedback)),
            )
            .into_any_element()
    }

    pub(super) fn render_status(&self, view: &SettingsView, cx: &mut Context<Self>) -> AnyElement {
        let current = self.settings.status;
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(page_title(t!("settings.status")))
            .child(
                div()
                    .mb_4()
                    .text_size(zoomed(14.))
                    .text_color(rgb(theme::text_muted()))
                    .child(t!("settings.status_hint")),
            )
            .children(Status::ALL.into_iter().map(|status| {
                let selected = current == Some(status);
                let description = match status {
                    Status::Online => None,
                    Status::Idle => None,
                    Status::Dnd => Some(t!("settings.dnd_hint")),
                    Status::Invisible => {
                        Some(t!("settings.invisible_hint"))
                    }
                };
                div()
                    .id(("status", status as usize))
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_3()
                    .py_3()
                    .rounded(theme::radius::card())
                    .border_1()
                    .border_color(if selected { rgb(theme::brand()) } else { theme::border() })
                    .cursor_pointer()
                    .when(selected, |row| row.bg(theme::selected()))
                    .when(!selected, |row| row.hover(|s| s.bg(theme::hover())))
                    .child(
                        div()
                            .flex_none()
                            .size(zoomed(12.))
                            .rounded_full()
                            .bg(rgb(widgets::status_color(status.as_str()))),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .child(
                                div()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(theme::text_strong()))
                                    .child(widgets::status_label(Some(status.as_str()))),
                            )
                            .children(description.map(|text| {
                                div()
                                    .text_size(zoomed(14.))
                                    .text_color(rgb(theme::text_muted()))
                                    .child(text)
                            })),
                    )
                    .when(selected, |row| {
                        row.child(div().text_color(rgb(theme::text_strong())).child("✓"))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| this.set_status(status, cx)))
            }))
            .child(self.render_custom_status(view, cx))
            .into_any_element()
    }
}

/// `nome@gmail.com` → `****@gmail.com`, as Discord shows it until revealed.
pub(super) fn mask_email(email: &str) -> String {
    match email.split_once('@') {
        Some((local, domain)) => format!("{}@{domain}", "*".repeat(local.chars().count())),
        None => "*".repeat(email.chars().count()),
    }
}

#[cfg(test)]
mod tests {
    use super::mask_email;

    #[test]
    fn masks_local_part_only() {
        assert_eq!(mask_email("nome@gmail.com"), "****@gmail.com");
        assert_eq!(mask_email("semarroba"), "*********");
    }
}
