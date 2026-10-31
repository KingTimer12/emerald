//! The window drawn: sidebars, the chat, panels and overlays.

use super::*;
use crate::zoom::zoomed;

impl Render for Emerald {
    #[hotpath::measure]
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Read here, not only on change: the window may start out focused.
        self.window_active = window.is_window_active();
        self.apply_appearance(window);
        self.images.release(window, cx);
        let width = f32::from(window.viewport_size().width);
        // Visual-refresh frame: the title bar, then the server list on the app
        // background and one bordered panel with a rounded top-left corner
        // holding sidebar and chat, and the user panel floating over the
        // bottom-left corner.
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
            Some(Overlay::ChannelSettings(modal)) => Some(self.render_channel_settings(modal, cx)),
            Some(Overlay::Switcher(switcher)) => Some(self.render_switcher(switcher, cx)),
            Some(Overlay::Confirm(ask)) => Some(self.render_confirm(*ask, cx)),
            Some(Overlay::Prompt(prompt)) => Some(self.render_prompt(*prompt, cx)),
            Some(Overlay::GroupPicker(picker)) => Some(self.render_group_picker(picker, cx)),
            Some(Overlay::Invite(modal)) => Some(self.render_invite_modal(modal, cx)),
            None => None,
        };
        let inbox = self.render_inbox(window.viewport_size().height, cx);
        let menu = self.render_message_menu(cx);
        let picker = self.render_reaction_picker(cx);
        let place_menu = self.render_place_menu(cx);
        let user_menu = self.render_user_menu(cx);
        let audio_menu = self.render_audio_menu(cx);
        let own_popover = self.render_own_popover(cx);
        let root = div()
            .track_focus(&self.focus_handle)
            .key_context("Emerald")
            .on_action(cx.listener(|this, _: &Dismiss, window, cx| this.dismiss(window, cx)))
            .on_action(cx.listener(|this, _: &EditLast, window, cx| this.edit_last(window, cx)))
            .on_action(cx.listener(|this, _: &QuickSwitch, window, cx| this.open_switcher(window, cx)))
            .on_action(cx.listener(|this, _: &MarkRead, _, cx| this.mark_read(cx)))
            .on_action(cx.listener(|this, _: &NextUnread, window, cx| this.step_channel(true, true, window, cx)))
            .on_action(cx.listener(|this, _: &ZoomIn, _, cx| this.zoom_by(true, cx)))
            .on_action(cx.listener(|this, _: &ZoomOut, _, cx| this.zoom_by(false, cx)))
            .on_action(cx.listener(|this, _: &ZoomReset, _, cx| this.zoom_reset(cx)))
            .on_action(cx.listener(|this, _: &PreviousChannel, window, cx| this.step_channel(false, false, window, cx)))
            .on_action(cx.listener(|this, _: &NextChannel, window, cx| this.step_channel(true, false, window, cx)))
            .on_action(cx.listener(|this, _: &GoBack, window, cx| this.go_back(window, cx)))
            .on_action(cx.listener(|this, _: &GoForward, window, cx| this.go_forward(window, cx)))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| this.record_shortcut(event, cx)))
            .on_action(cx.listener(|this, _: &crate::text_input::Copy, _, cx| {
                this.copy_selection(cx);
            }))
            .on_action(cx.listener(|this, _: &crate::Logout, _, cx| this.log_out(cx)))
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(theme::base_lowest()))
            .font_family(theme::fonts::sans())
            .text_size(zoomed(16.))
            // gpui defaults to 1.618; Discord's interface sits near 1.25.
            .line_height(relative(1.25))
            .text_color(theme::text())
            .child(self.render_title_bar(window, cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(zoomed(0.))
                    .child(self.render_servers(cx))
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .min_w(zoomed(0.))
                            .h_full()
                            .rounded_tl(theme::radius::card())
                            .border_l_1()
                            .border_t_1()
                            .border_color(theme::border())
                            .child(self.render_channels(cx))
                            .child(self.render_chat(width, cx)),
                    ),
            )
            .children(self.render_call_panel(cx))
            .child(self.render_user_panel(cx))
            .children(overlay)
            .children(inbox)
            .children(menu)
            .children(picker)
            .children(place_menu)
            .children(user_menu)
            .children(audio_menu)
            .children(own_popover);
        // A profile effect drawn above has layers on a clock.
        if self.animating.take() {
            self.next_animation_frame(cx);
        }
        if self.typing_animating.take() {
            self.next_typing_frame(cx);
        }
        root
    }
}
