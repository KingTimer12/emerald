//! The window drawn: sidebars, the chat, panels and overlays.

use super::*;

impl Render for Emerald {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Read here, not only on change: the window may start out focused.
        self.window_active = window.is_window_active();
        for image in self.images.take_retired() {
            cx.drop_image(image, Some(window));
        }
        let wide = f32::from(window.viewport_size().width) >= PROFILE_MIN_WINDOW;
        // Visual-refresh frame: server list on the app background, then one
        // bordered panel with a rounded top-left corner holding sidebar and
        // chat, and the user panel floating over the bottom-left corner.
        let overlay = match &self.overlay {
            Some(Overlay::AddServer(modal)) => Some(self.render_add_server(modal, cx).into_any_element()),
            Some(Overlay::Settings(view)) => Some(self.render_settings(view, cx).into_any_element()),
            Some(Overlay::Profile { user, guild }) => {
                Some(self.render_profile_modal(*user, *guild, cx).into_any_element())
            }
            Some(Overlay::DeleteMessage { channel, message }) => {
                Some(self.render_delete_modal(*channel, *message, cx).into_any_element())
            }
            Some(Overlay::EditAttachment { ix, spoiler }) => self.render_attachment_modal(*ix, *spoiler, cx),
            Some(Overlay::CreateThread(modal)) => Some(self.render_thread_modal(modal, cx)),
            Some(Overlay::CreatePost(modal)) => Some(self.render_post_modal(modal, cx)),
            Some(Overlay::CreatePoll(modal)) => Some(self.render_poll_modal(modal, cx)),
            None => None,
        };
        let menu = self.render_message_menu(cx);
        let picker = self.render_reaction_picker(cx);
        div()
            .track_focus(&self.focus_handle)
            .key_context("Emerald")
            .on_action(cx.listener(|this, _: &Dismiss, window, cx| this.dismiss(window, cx)))
            .on_action(cx.listener(|this, _: &EditLast, window, cx| this.edit_last(window, cx)))
            .on_action(cx.listener(|this, _: &crate::text_input::Copy, _, cx| {
                this.copy_selection(cx);
            }))
            .on_action(cx.listener(|this, _: &crate::Logout, _, cx| this.log_out(cx)))
            .relative()
            .flex()
            .size_full()
            .bg(rgb(theme::BASE_LOWEST))
            .text_size(px(16.))
            // gpui defaults to 1.618; Discord's interface sits near 1.25.
            .line_height(relative(1.25))
            .text_color(theme::text())
            .child(self.render_servers(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .rounded_tl(px(8.))
                    .border_l_1()
                    .border_t_1()
                    .border_color(theme::border())
                    .child(self.render_channels(cx))
                    .child(self.render_chat(wide, cx)),
            )
            .child(self.render_user_panel(cx))
            .children(overlay)
            .children(menu)
            .children(picker)
    }
}
