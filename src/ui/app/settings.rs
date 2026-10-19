//! Full-screen user settings, laid out like Discord's: category sidebar on
//! the left, the open page in the middle and the ESC close button on the right.

use gpui::{
    AnyElement, Context, Entity, FontWeight, SharedString, Subscription, Window, div, prelude::*,
    px, rgb,
};

use super::super::text_input::{Submitted, TextInput};
use super::super::theme;
use super::widgets::{self, ButtonKind};
use super::{Emerald, Overlay};
use crate::discord::model::{Snowflake, Status, avatar_url};
use crate::discord::{Action, Command};
use crate::settings::FONT_SIZES;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Account,
    Status,
    Appearance,
}

pub struct SettingsView {
    page: Page,
    display_name: Entity<TextInput>,
    reveal_email: bool,
    pub feedback: Option<super::Feedback>,
    _subscription: Subscription,
}

impl Emerald {
    pub(super) fn open_settings(&mut self, page: Page, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(Overlay::Settings(view)) = &mut self.overlay {
            view.page = page;
            cx.notify();
            return;
        }
        let current = self
            .people
            .me()
            .and_then(|me| me.global_name.clone())
            .unwrap_or_default();
        let display_name = cx.new(|cx| {
            let mut input = TextInput::new("Nome exibido", cx).keep_on_submit();
            input.set_text(current, cx);
            input
        });
        let subscription = cx.subscribe(&display_name, |this, _, event: &Submitted, cx| {
            this.save_display_name(event.0.clone(), cx)
        });
        self.overlay = Some(Overlay::Settings(SettingsView {
            page,
            display_name,
            reveal_email: false,
            feedback: None,
            _subscription: subscription,
        }));
        window.focus(&self.focus_handle);
        cx.notify();
    }

    fn settings_view(&mut self) -> Option<&mut SettingsView> {
        match &mut self.overlay {
            Some(Overlay::Settings(view)) => Some(view),
            _ => None,
        }
    }

    /// An empty name resets the display name to the username.
    fn save_display_name(&mut self, name: String, cx: &mut Context<Self>) {
        let name = name.trim();
        let name = (!name.is_empty()).then(|| name.to_owned());
        if let Some(view) = self.settings_view() {
            view.feedback = None;
        }
        self.run(Action::UpdateProfile, Command::SetGlobalName { name }, cx);
    }

    fn set_status(&mut self, status: Status, cx: &mut Context<Self>) {
        self.settings.status = Some(status);
        self.net.post(Command::SetStatus(status));
        self.save_settings(cx);
    }

    pub(super) fn render_settings(&self, view: &SettingsView, cx: &mut Context<Self>) -> impl IntoElement {
        let item = |id: &'static str, label: &'static str, page: Page, cx: &mut Context<Self>| {
            let selected = view.page == page;
            div()
                .id(id)
                .px_2p5()
                .py(px(6.))
                .rounded_md()
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .text_color(rgb(if selected { theme::TEXT_STRONG } else { theme::INTERACTIVE }))
                .when(selected, |i| i.bg(theme::selected()))
                .when(!selected, |i| {
                    i.hover(|s| s.bg(theme::hover()).text_color(rgb(theme::TEXT_STRONG)))
                })
                .child(label)
                .on_click(cx.listener(move |this, _, window, cx| this.open_settings(page, window, cx)))
        };
        let separator = || div().mx_2p5().my_2().h(px(1.)).bg(theme::border());

        let sidebar = div()
            .id("settings-sidebar")
            .w(px(260.))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(2.))
            .pt(px(60.))
            .px_3()
            .overflow_y_scroll()
            .bg(rgb(theme::BASE_LOWEST))
            .child(widgets::label("CONFIGURAÇÕES DE USUÁRIO").px_2p5().pb_1())
            .child(item("page-account", "Minha conta", Page::Account, cx))
            .child(item("page-status", "Status", Page::Status, cx))
            .child(separator())
            .child(widgets::label("CONFIGURAÇÕES DO APLICATIVO").px_2p5().pb_1())
            .child(item("page-appearance", "Aparência", Page::Appearance, cx))
            .child(separator())
            .child(
                div()
                    .id("settings-logout")
                    .px_2p5()
                    .py(px(6.))
                    .rounded_md()
                    .font_weight(FontWeight::MEDIUM)
                    .cursor_pointer()
                    .text_color(rgb(theme::DANGER))
                    .hover(|s| s.bg(theme::hover()))
                    .child("Sair")
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(crate::Logout), cx)),
            );

        let page = match view.page {
            Page::Account => self.render_account(view, cx),
            Page::Status => self.render_status(cx),
            Page::Appearance => self.render_appearance(cx),
        };

        let close = div()
            .w(px(72.))
            .flex_none()
            .pt(px(60.))
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .child(
                div()
                    .id("close-settings")
                    .size(px(36.))
                    .rounded_full()
                    .border_2()
                    .border_color(rgb(theme::INTERACTIVE))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(theme::INTERACTIVE))
                    .cursor_pointer()
                    .hover(|s| s.border_color(rgb(theme::TEXT_STRONG)).text_color(rgb(theme::TEXT_STRONG)))
                    .child("✕")
                    .on_click(cx.listener(|this, _, window, cx| this.close_overlay(window, cx))),
            )
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(theme::INTERACTIVE))
                    .child("ESC"),
            );

        div()
            .id("settings")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .bg(rgb(theme::BASE_LOWER))
            .occlude()
            .child(sidebar)
            .child(
                div()
                    .id("settings-page")
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .overflow_y_scroll()
                    .pt(px(60.))
                    .px_10()
                    .pb_10()
                    .child(div().max_w(px(740.)).child(page)),
            )
            .child(close)
    }

    fn render_account(&self, view: &SettingsView, cx: &mut Context<Self>) -> AnyElement {
        let Some(me) = self.people.me() else {
            return page_title("Minha conta").into_any_element();
        };
        let name: SharedString = me.display_name().to_owned().into();
        let email: SharedString = match (&me.email, view.reveal_email) {
            (None, _) => "Nenhum e-mail".into(),
            (Some(email), true) => email.clone().into(),
            (Some(email), false) => mask_email(email).into(),
        };
        let field = |caption: &'static str, value: SharedString| {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(widgets::label(caption))
                .child(div().text_color(rgb(theme::TEXT_DEFAULT)).child(value))
        };
        let busy = self.busy.contains(&Action::UpdateProfile);

        div()
            .flex()
            .flex_col()
            .child(page_title("Minha conta"))
            .child(
                div()
                    .rounded_lg()
                    .overflow_hidden()
                    .bg(rgb(theme::BASE_LOWEST))
                    .child(div().h(px(100.)).bg(rgb(theme::BRAND)))
                    .child(
                        div()
                            .flex()
                            .items_end()
                            .gap_4()
                            .px_4()
                            .child(
                                // Ring around the avatar where it overlaps the banner.
                                div()
                                    .mt(px(-40.))
                                    .p(px(6.))
                                    .rounded_full()
                                    .bg(rgb(theme::BASE_LOWEST))
                                    .child(widgets::avatar(
                                        &name,
                                        me.id,
                                        self.avatar_image(&me.avatar_url()),
                                        80.,
                                        self.own_status().map(|s| (s, theme::BASE_LOWEST)),
                                    )),
                            )
                            .child(
                                div()
                                    .pb_3()
                                    .text_size(px(20.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(theme::TEXT_STRONG))
                                    .child(name.clone()),
                            ),
                    )
                    .child(
                        div()
                            .m_4()
                            .p_4()
                            .rounded_lg()
                            .bg(rgb(theme::BASE_LOW))
                            .flex()
                            .flex_col()
                            .gap_6()
                            .child(field("NOME EXIBIDO", name))
                            .child(field("NOME DE USUÁRIO", me.username.clone().into()))
                            .child(
                                div()
                                    .flex()
                                    .items_end()
                                    .gap_1()
                                    .child(field("E-MAIL", email))
                                    .when(me.email.is_some(), |row| {
                                        row.child(
                                            div()
                                                .id("reveal-email")
                                                .text_size(px(14.))
                                                .text_color(rgb(theme::LINK))
                                                .cursor_pointer()
                                                .hover(|s| s.underline())
                                                .child(if view.reveal_email { "Ocultar" } else { "Mostrar" })
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
                    .child(section_title("Editar nome exibido"))
                    .child(
                        div()
                            .text_size(px(14.))
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child("É assim que os outros te veem. Deixe em branco para usar o nome de usuário."),
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
                                    if busy { "Salvando…" } else { "Salvar" },
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

    fn render_status(&self, cx: &mut Context<Self>) -> AnyElement {
        let current = self.settings.status;
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(page_title("Status"))
            .child(
                div()
                    .mb_4()
                    .text_size(px(14.))
                    .text_color(rgb(theme::TEXT_MUTED))
                    .child(
                        "Escolha como você aparece para os outros. A escolha fica salva neste computador \
                         e é enviada ao Discord a cada conexão; se o app oficial estiver aberto ao mesmo \
                         tempo, ele pode sobrescrevê-la.",
                    ),
            )
            .children(Status::ALL.into_iter().map(|status| {
                let selected = current == Some(status);
                let description = match status {
                    Status::Online => None,
                    Status::Idle => None,
                    Status::Dnd => Some("Você não receberá notificações de mensagens."),
                    Status::Invisible => {
                        Some("Você não aparecerá online, mas terá acesso total ao Discord.")
                    }
                };
                div()
                    .id(("status", status as usize))
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_3()
                    .py_3()
                    .rounded_lg()
                    .border_1()
                    .border_color(if selected { rgb(theme::BRAND).into() } else { theme::border() })
                    .cursor_pointer()
                    .when(selected, |row| row.bg(theme::selected()))
                    .when(!selected, |row| row.hover(|s| s.bg(theme::hover())))
                    .child(
                        div()
                            .flex_none()
                            .size(px(12.))
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
                                    .text_color(rgb(theme::TEXT_STRONG))
                                    .child(widgets::status_label(Some(status.as_str()))),
                            )
                            .children(description.map(|text| {
                                div()
                                    .text_size(px(14.))
                                    .text_color(rgb(theme::TEXT_MUTED))
                                    .child(text)
                            })),
                    )
                    .when(selected, |row| {
                        row.child(div().text_color(rgb(theme::TEXT_STRONG)).child("✓"))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| this.set_status(status, cx)))
            }))
            .into_any_element()
    }

    fn render_appearance(&self, cx: &mut Context<Self>) -> AnyElement {
        let size = self.settings.font_size;
        let compact = self.settings.compact;
        let preview_text = "Assim ficam as mensagens no chat.";
        let preview = div()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(theme::border())
            .bg(rgb(theme::BASE_LOWER))
            .text_size(px(size))
            .line_height(px((size * 1.375).round()))
            .child(if compact {
                div()
                    .flex()
                    .gap_2()
                    .child(div().text_xs().text_color(theme::muted()).child("12:00"))
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(theme::TEXT_STRONG))
                            .child("Wumpus"),
                    )
                    .child(div().child(preview_text))
            } else {
                div()
                    .flex()
                    .gap_4()
                    .child(widgets::avatar(
                        "Wumpus",
                        Snowflake(0),
                        self.avatar_image(&avatar_url(Snowflake(0), None)),
                        40.,
                        None,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(theme::TEXT_STRONG))
                                    .child("Wumpus"),
                            )
                            .child(div().child(preview_text)),
                    )
            });

        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(page_title("Aparência"))
            .child(preview)
            .child(section_title("Tamanho da fonte do chat").mt_6())
            .child(
                div().flex().gap_2().children(FONT_SIZES.into_iter().enumerate().map(|(ix, option)| {
                    let selected = option == size;
                    div()
                        .id(("font-size", ix))
                        .h(px(32.))
                        .px_3()
                        .flex()
                        .items_center()
                        .rounded_md()
                        .text_size(px(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .cursor_pointer()
                        .text_color(rgb(theme::TEXT_STRONG))
                        .when(selected, |chip| chip.bg(rgb(theme::BRAND)))
                        .when(!selected, |chip| chip.bg(theme::hover()).hover(|s| s.bg(theme::active())))
                        .child(format!("{option}px"))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.settings.font_size = option;
                            this.save_settings(cx);
                        }))
                })),
            )
            .child(
                div()
                    .id("compact-mode")
                    .mt_6()
                    .flex()
                    .items_center()
                    .gap_4()
                    .cursor_pointer()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .child(section_title("Modo compacto"))
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .text_color(rgb(theme::TEXT_MUTED))
                                    .child("Mais mensagens na tela, em uma linha cada e sem avatares."),
                            ),
                    )
                    .child(widgets::toggle("compact-toggle", compact))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.settings.compact = !this.settings.compact;
                        this.save_settings(cx);
                    })),
            )
            .into_any_element()
    }
}

fn page_title(text: &'static str) -> gpui::Div {
    div()
        .mb_5()
        .text_size(px(20.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(theme::TEXT_STRONG))
        .child(text)
}

fn section_title(text: &'static str) -> gpui::Div {
    div()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(theme::TEXT_STRONG))
        .child(text)
}

/// `nome@gmail.com` → `****@gmail.com`, as Discord shows it until revealed.
fn mask_email(email: &str) -> String {
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
