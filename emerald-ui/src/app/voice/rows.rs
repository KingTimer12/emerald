//! Someone in a voice channel, listed under it in the sidebar: their
//! avatar ringed green while they speak in our call, their name, and
//! whether they muted, deafened or share their screen.

use gpui::{AnyElement, Context, FontWeight, MouseButton, SharedString, Stateful, div, prelude::*, rgb};

use super::super::super::{icons, theme};
use super::super::state::spaces::VoiceRow;
use super::super::{Emerald, widgets};
use emerald_discord::model::Snowflake;
use crate::zoom::zoomed;

/// Avatar size in a voice row.
const AVATAR: f32 = 24.;

impl Emerald {
    pub(in crate::app) fn render_voice_row(&self, row: Stateful<gpui::Div>, voice: &VoiceRow, cx: &mut Context<Self>) -> AnyElement {
        let user = voice.user;
        let guild = self.spaces.current().id;
        let (nick, color) = self.spaces.guild(guild).map_or((None, None), |g| g.member_style(user));
        let name: SharedString = nick
            .or_else(|| self.people.display_name(user))
            .unwrap_or_else(|| t!("voice.someone"))
            .to_owned()
            .into();
        let speaking = self.is_speaking(user);
        let avatar = widgets::avatar(&name, user, self.user_avatar(user), AVATAR, None);
        row.ml(zoomed(26.))
            .gap_2()
            .rounded_md()
            .cursor_pointer()
            .text_size(zoomed(14.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(theme::channels_default()))
            .hover(|s| s.bg(theme::hover()).text_color(rgb(theme::text_strong())))
            .child(
                // The ring sits around the avatar without moving it.
                div()
                    .flex_none()
                    .rounded_full()
                    .border_2()
                    .border_color(if speaking { rgb(theme::online()) } else { gpui::transparent_black().into() })
                    .child(avatar),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(zoomed(0.))
                    .truncate()
                    .when_some(color, |name, color| name.text_color(rgb(color)))
                    .child(name),
            )
            .when(voice.streaming, |row| {
                row.child(
                    div()
                        .px_1()
                        .rounded_sm()
                        .bg(rgb(theme::danger()))
                        .text_size(zoomed(10.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(gpui::white())
                        .child(t!("voice.live")),
                )
            })
            .when(voice.muted && !voice.deafened, |row| row.child(icons::icon(icons::MICROPHONE_OFF, 16., theme::interactive_muted())))
            .when(voice.deafened, |row| row.child(icons::icon(icons::HEADPHONES_OFF, 16., theme::interactive_muted())))
            .on_click(cx.listener(move |this, _, window, cx| this.open_profile(user, Some(guild), window, cx)))
            .on_mouse_down(MouseButton::Right, self.on_user_right_click(user, Some(guild), None, cx))
            .into_any_element()
    }

    /// Names of the people in voice that are not known yet, asked for when
    /// a server opens.
    pub(in crate::app) fn request_voice_members(&mut self, guild: Snowflake) {
        let Some(space) = self.spaces.iter().find(|s| s.id == guild) else { return };
        let users: Vec<Snowflake> = space
            .voice_users()
            .filter(|user| self.people.user(*user).is_none())
            .collect();
        let Some(info) = self.spaces.guild_mut(guild) else { return };
        let missing = info.unrequested(users.into_iter());
        for users in missing.chunks(super::super::navigation::MEMBERS_PER_REQUEST) {
            self.net.post(emerald_discord::Command::RequestMembers { guild, users: users.to_vec() });
        }
    }
}
