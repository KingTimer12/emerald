//! "Add a Server" modal behind the "+" in the server list: pick between
//! creating a server and joining one with an invite, like the web client.

use gpui::{
    Context, Entity, Focusable, FontWeight, MouseButton, SharedString, Subscription, Window, div, prelude::*,
    px, rgb,
};

use super::super::text_input::{Submitted, TextInput};
use super::super::theme;
use super::widgets::{self, ButtonKind};
use super::{Emerald, Overlay};
use crate::discord::rest::invite_code;
use crate::discord::{Action, Command};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Choose,
    Create,
    Join,
}

pub struct AddServer {
    step: Step,
    name: Entity<TextInput>,
    invite: Entity<TextInput>,
    pub error: Option<SharedString>,
    _subscriptions: [Subscription; 2],
}

impl Emerald {
    pub(super) fn open_add_server(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let owner = self
            .people
            .me()
            .map_or_else(String::new, |me| me.display_name().to_owned());
        let name = cx.new(|cx| {
            let mut input = TextInput::new("Nome do servidor", cx).keep_on_submit();
            input.set_text(format!("Servidor de {owner}"), cx);
            input
        });
        let invite = cx.new(|cx| TextInput::new("https://discord.gg/hTKzmak", cx).keep_on_submit());
        let subscriptions = [
            cx.subscribe(&name, |this, _, event: &Submitted, cx| {
                this.create_guild(event.0.clone(), cx)
            }),
            cx.subscribe(&invite, |this, _, event: &Submitted, cx| {
                this.join_guild(event.0.clone(), cx)
            }),
        ];
        self.overlay = Some(Overlay::AddServer(AddServer {
            step: Step::Choose,
            name,
            invite,
            error: None,
            _subscriptions: subscriptions,
        }));
        window.focus(&self.focus_handle);
        cx.notify();
    }

    fn add_server_step(&mut self, step: Step, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Overlay::AddServer(modal)) = &mut self.overlay else { return };
        modal.step = step;
        modal.error = None;
        match step {
            Step::Choose => window.focus(&self.focus_handle),
            Step::Create => window.focus(&modal.name.focus_handle(cx)),
            Step::Join => window.focus(&modal.invite.focus_handle(cx)),
        }
        cx.notify();
    }

    fn set_add_server_error(&mut self, error: &str, cx: &mut Context<Self>) {
        if let Some(Overlay::AddServer(modal)) = &mut self.overlay {
            modal.error = Some(error.to_owned().into());
            cx.notify();
        }
    }

    fn create_guild(&mut self, name: String, cx: &mut Context<Self>) {
        let name = name.trim().to_owned();
        if name.is_empty() {
            return self.set_add_server_error("Dê um nome ao seu servidor.", cx);
        }
        if let Some(Overlay::AddServer(modal)) = &mut self.overlay {
            modal.error = None;
        }
        if self.run(Action::CreateGuild, Command::CreateGuild { name }, cx) {
            self.select_new_guild = true;
        }
    }

    fn join_guild(&mut self, input: String, cx: &mut Context<Self>) {
        let Some(code) = invite_code(&input) else {
            return self.set_add_server_error("Insira um link ou código de convite válido.", cx);
        };
        if let Some(Overlay::AddServer(modal)) = &mut self.overlay {
            modal.error = None;
        }
        if self.run(Action::JoinGuild, Command::JoinGuild { code }, cx) {
            self.select_new_guild = true;
        }
    }

    pub(super) fn render_add_server(&self, modal: &AddServer, cx: &mut Context<Self>) -> impl IntoElement {
        let title = |text: &'static str| {
            div()
                .text_size(px(24.))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(theme::TEXT_STRONG))
                .child(text)
        };
        let subtitle = |text: &'static str| {
            div()
                .text_color(rgb(theme::TEXT_MUTED))
                .text_center()
                .child(text)
        };
        let error = modal.error.clone().map(|error| {
            div()
                .text_size(px(14.))
                .text_color(rgb(theme::FEEDBACK_CRITICAL))
                .child(error)
        });

        let (body, footer) = match modal.step {
            Step::Choose => (
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_2()
                    .child(title("Crie seu servidor"))
                    .child(subtitle(
                        "Seu servidor é onde você e seus amigos se reúnem. Crie o seu e comece a conversar.",
                    ))
                    .child(
                        div()
                            .id("create-own")
                            .mt_4()
                            .w_full()
                            .h(px(64.))
                            .px_4()
                            .flex()
                            .items_center()
                            .justify_between()
                            .rounded_lg()
                            .border_1()
                            .border_color(theme::border())
                            .cursor_pointer()
                            .hover(|s| s.bg(theme::hover()))
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(theme::TEXT_STRONG))
                                    .child("Criar o meu"),
                            )
                            .child(div().text_size(px(20.)).text_color(rgb(theme::INTERACTIVE)).child("›"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.add_server_step(Step::Create, window, cx)
                            })),
                    ),
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .text_size(px(20.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::TEXT_STRONG))
                            .child("Já tem um convite?"),
                    )
                    .child(
                        widgets::button("go-join", "Entrar em um servidor", ButtonKind::Secondary)
                            .w_full()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.add_server_step(Step::Join, window, cx)
                            })),
                    ),
            ),
            Step::Create => {
                let busy = self.busy.contains(&Action::CreateGuild);
                (
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_2()
                        .child(title("Personalize o seu servidor"))
                        .child(subtitle(
                            "Deixe o seu novo servidor com a sua cara dando um nome a ele. Você pode mudar isso depois.",
                        ))
                        .child(
                            div()
                                .mt_4()
                                .w_full()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(widgets::label("NOME DO SERVIDOR"))
                                .child(modal.name.clone())
                                .children(error),
                        ),
                    footer(
                        cx,
                        Step::Choose,
                        widgets::button("create", if busy { "Criando…" } else { "Criar" }, ButtonKind::Brand)
                            .when(busy, |b| b.opacity(0.5))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let Some(Overlay::AddServer(modal)) = &this.overlay else { return };
                                let name = modal.name.read(cx).text().to_owned();
                                this.create_guild(name, cx);
                            })),
                    ),
                )
            }
            Step::Join => {
                let busy = self.busy.contains(&Action::JoinGuild);
                let example = |text: &'static str| {
                    div()
                        .text_size(px(14.))
                        .text_color(rgb(theme::TEXT_DEFAULT))
                        .child(text)
                };
                (
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_2()
                        .child(title("Entrar em um servidor"))
                        .child(subtitle(
                            "Insira um convite abaixo para entrar em um servidor existente.",
                        ))
                        .child(
                            div()
                                .mt_4()
                                .w_full()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(widgets::label("LINK DO CONVITE"))
                                .child(modal.invite.clone())
                                .children(error)
                                .child(widgets::label("OS CONVITES DEVEM SE PARECER COM").mt_3())
                                .child(example("hTKzmak"))
                                .child(example("https://discord.gg/hTKzmak"))
                                .child(example("https://discord.gg/pessoas-legais")),
                        ),
                    footer(
                        cx,
                        Step::Choose,
                        widgets::button(
                            "join",
                            if busy { "Entrando…" } else { "Entrar no servidor" },
                            ButtonKind::Brand,
                        )
                        .when(busy, |b| b.opacity(0.5))
                        .on_click(cx.listener(|this, _, _, cx| {
                            let Some(Overlay::AddServer(modal)) = &this.overlay else { return };
                            let input = modal.invite.read(cx).text().to_owned();
                            this.join_guild(input, cx);
                        })),
                    ),
                )
            }
        };

        // Scrim: clicking outside the panel closes the modal.
        div()
            .id("add-server-scrim")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme::scrim())
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.close_overlay(window, cx)),
            )
            .child(
                div()
                    .id("add-server")
                    .relative()
                    .w(px(440.))
                    .rounded_lg()
                    .overflow_hidden()
                    .border_1()
                    .border_color(theme::border())
                    .bg(rgb(theme::SURFACE_HIGH))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .absolute()
                            .top_3()
                            .right_3()
                            .child(widgets::icon_button("close-add-server", "✕").on_click(
                                cx.listener(|this, _, window, cx| this.close_overlay(window, cx)),
                            )),
                    )
                    .child(div().px_4().pt_6().pb_4().child(body))
                    .child(div().p_4().bg(rgb(theme::BASE_LOW)).child(footer)),
            )
    }
}

/// "Voltar" on the left, the step's primary button on the right.
fn footer(cx: &mut Context<Emerald>, back: Step, primary: gpui::Stateful<gpui::Div>) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .child(
            widgets::link_button("back", "Voltar").on_click(cx.listener(move |this, _, window, cx| {
                this.add_server_step(back, window, cx)
            })),
        )
        .child(primary)
}
