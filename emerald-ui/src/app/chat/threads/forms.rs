//! The Create Thread and New Post forms.

use super::*;
use crate::zoom::zoomed;

impl Emerald {
    pub(in super::super::super) fn render_thread_modal(&self, modal: &NewThread, cx: &mut Context<Self>) -> AnyElement {
        let busy = self.busy.contains(&Action::CreateThread);
        let body = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(widgets::label(t!("thread.name")))
            .child(modal.name.clone());
        let submit = widgets::button("thread-create", t!("thread.create_button"), widgets::ButtonKind::Brand)
            .when(busy, |button| button.opacity(0.5))
            .on_click(cx.listener(|this, _, _, cx| this.create_thread(cx)));
        self.form_modal("thread-modal", t!("thread.create"), body, modal.error.clone(), submit, cx)
    }

    pub(in super::super::super) fn render_post_modal(&self, modal: &NewPost, cx: &mut Context<Self>) -> AnyElement {
        let busy = self.busy.contains(&Action::CreateThread);
        let space = self.spaces.current();
        let manages = space.permissions(modal.forum) & permissions::MANAGE_THREADS != 0;
        let available = space
            .channels()
            .iter()
            .find(|c| c.id == modal.forum)
            .map_or(&[][..], |f| f.available_tags.as_slice());
        let tags = (!available.is_empty()).then(|| {
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(widgets::label(t!("forum.tags")))
                .child(div().flex().flex_wrap().gap_1().children(
                    available.iter().filter(|t| manages || !t.moderated).enumerate().map(|(ix, t)| {
                        let id = t.id;
                        let on = modal.tags.contains(&id);
                        let label = match &t.emoji_name {
                            Some(emoji) => format!("{emoji} {}", t.name),
                            None => t.name.clone(),
                        };
                        div()
                            .id(("post-tag", ix))
                            .px_2()
                            .py(zoomed(2.))
                            .rounded_full()
                            .border_1()
                            .border_color(if on { rgb(theme::brand()) } else { theme::border() })
                            .bg(if on { theme::active() } else { rgb(theme::base_lower()) })
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(if on { theme::text_strong() } else { theme::text_default() }))
                            .cursor_pointer()
                            .hover(|s| s.bg(theme::hover()))
                            .child(label)
                            .on_click(cx.listener(move |this, _, _, cx| this.toggle_post_tag(id, cx)))
                    }),
                ))
        });
        let body = div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(widgets::label(t!("forum.title")))
                    .child(modal.title.clone()),
            )
            .children(tags)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(widgets::label(t!("forum.content")))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .children(self.render_attachments(false, cx))
                            .child(
                                div()
                                    .flex()
                                    .items_start()
                                    .gap_1()
                                    .children(self.render_attach_button(cx))
                                    .child(div().flex_1().min_w(zoomed(0.)).child(modal.content.clone())),
                            ),
                    ),
            );
        let submit = widgets::button("post-create", t!("forum.post_button"), widgets::ButtonKind::Brand)
            .when(busy, |button| button.opacity(0.5))
            .on_click(cx.listener(|this, _, _, cx| this.create_post(cx)));
        self.form_modal("post-modal", t!("forum.new_post"), body, modal.error.clone(), submit, cx)
    }

    /// A centered form over a scrim: title, fields, the error, then Cancel
    /// and `submit`.
    pub(in super::super) fn form_modal(
        &self,
        id: &'static str,
        title: &'static str,
        body: gpui::Div,
        error: Option<SharedString>,
        submit: gpui::Stateful<gpui::Div>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let modal = div()
            .id(id)
            .w(zoomed(440.))
            .rounded(theme::radius::modal())
            .overflow_hidden()
            .border_1()
            .border_color(theme::border())
            .bg(rgb(theme::surface_high()))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .p_4()
                    .child(
                        div()
                            .text_size(zoomed(20.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::text_strong()))
                            .child(title),
                    )
                    .child(body)
                    .children(error.map(|error| {
                        div().text_size(zoomed(14.)).text_color(rgb(theme::feedback_critical())).child(error)
                    })),
            )
            .child(
                div()
                    .p_4()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .bg(rgb(theme::base_low()))
                    .child(
                        widgets::button((id, 0usize), t!("common.cancel"), widgets::ButtonKind::Secondary)
                            .on_click(cx.listener(|this, _, window, cx| this.close_overlay(window, cx))),
                    )
                    .child(submit),
            );
        div()
            .id((id, 1usize))
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme::scrim())
            .occlude()
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| this.close_overlay(window, cx)))
            .child(modal)
            .into_any_element()
    }
}
