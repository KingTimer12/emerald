//! The place menu's entries beyond reading and notifications: inviting
//! people, leaving a server, closing a DM and running a group DM.

use std::sync::Arc;

use gpui::{AnyElement, Context, PathPromptOptions, div, prelude::*, px, rgb};

use super::super::theme;
use super::Emerald;
use super::dialogs::{Ask, Prompt};
use super::place_menu::{Target, item};
use emerald_discord::Command;
use emerald_discord::model::{ChannelKind, Snowflake};
use emerald_discord::permissions;

/// Discord refuses group icons larger than this.
const ICON_LIMIT: usize = 8 * 1024 * 1024;
/// Bit of "Create Invite".
const CREATE_INVITE: u64 = 1;

impl Emerald {
    /// A channel of `guild` the user may invite to: the open one, else the
    /// first listed, as "Invite People" on a server picks.
    fn invite_channel(&self, guild: Snowflake) -> Option<Snowflake> {
        let space = self.spaces.iter().find(|s| s.id == guild)?;
        let allowed = |id: Snowflake| {
            let granted = space.permissions(id);
            granted & permissions::ADMINISTRATOR != 0 || granted & (CREATE_INVITE | permissions::VIEW_CHANNEL) == CREATE_INVITE | permissions::VIEW_CHANNEL
        };
        self.selected_channel
            .filter(|id| space.row(*id).is_some_and(|row| !row.kind.is_thread()))
            .into_iter()
            .chain(space.first_openable())
            .find(|id| allowed(*id))
    }

    pub(super) fn more_place_items(&self, target: Target, items: &mut Vec<AnyElement>, cx: &mut Context<Self>) {
        let separator = || div().my_1().h(px(1.)).bg(theme::border()).into_any_element();
        let danger = |id: &'static str, label: &'static str| item((id, 0), label.into(), None).text_color(rgb(theme::FEEDBACK_CRITICAL));
        match target {
            Target::Guild(guild) => {
                items.push(separator());
                if let Some(channel) = self.invite_channel(guild) {
                    items.push(
                        item(("menu-invite", 0), t!("invite.invite_people").into(), None)
                            .on_click(cx.listener(move |this, _, _, cx| this.open_invite(channel, "Guild Context Menu", cx)))
                            .into_any_element(),
                    );
                }
                let owner = self.spaces.guild(guild).and_then(|g| g.owner());
                if owner != Some(self.people.my_id()) {
                    items.push(
                        danger("menu-leave", t!("leave.server"))
                            .on_click(cx.listener(move |this, _, _, cx| this.ask(Ask::LeaveGuild(guild), cx)))
                            .into_any_element(),
                    );
                }
            }
            Target::Channel {
                guild: Some(guild),
                channel,
                category: false,
                thread: false,
            } => {
                if self.invite_channel(guild).is_some_and(|_| {
                    let granted = self.spaces.iter().find(|s| s.id == guild).map_or(0, |s| s.permissions(channel));
                    granted & (CREATE_INVITE | permissions::ADMINISTRATOR) != 0
                }) {
                    items.push(separator());
                    items.push(
                        item(("menu-invite", 0), t!("invite.invite_people").into(), None)
                            .on_click(cx.listener(move |this, _, _, cx| this.open_invite(channel, "Context Menu", cx)))
                            .into_any_element(),
                    );
                }
            }
            Target::Channel { guild: None, channel, .. } => {
                let group = self.spaces.channel(channel).is_some_and(|c| c.kind == ChannelKind::GROUP_DM);
                items.push(separator());
                if group {
                    items.push(
                        item(("menu-rename", 0), t!("group.rename").into(), None)
                            .on_click(cx.listener(move |this, _, window, cx| this.prompt(Prompt::RenameGroup(channel), window, cx)))
                            .into_any_element(),
                    );
                    items.push(
                        item(("menu-icon", 0), t!("group.change_icon").into(), None)
                            .on_click(cx.listener(move |this, _, _, cx| this.pick_group_icon(channel, cx)))
                            .into_any_element(),
                    );
                    items.push(
                        danger("menu-leave", t!("leave.group"))
                            .on_click(cx.listener(move |this, _, _, cx| this.ask(Ask::LeaveGroup(channel), cx)))
                            .into_any_element(),
                    );
                } else {
                    items.push(
                        item(("menu-close", 0), t!("dm.close").into(), None)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.place_menu = None;
                                this.net.post(Command::CloseChannel { channel });
                                cx.notify();
                            }))
                            .into_any_element(),
                    );
                }
            }
            Target::Channel { .. } => {}
        }
    }

    /// "Change Icon": an image from disk, sent as the group's icon.
    fn pick_group_icon(&mut self, channel: Snowflake, cx: &mut Context<Self>) {
        self.place_menu = None;
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else { return };
            let Some(path) = paths.into_iter().next() else { return };
            let mime = match path.extension().and_then(|e| e.to_str()).map(str::to_lowercase).as_deref() {
                Some("png") => "image/png",
                Some("jpg" | "jpeg") => "image/jpeg",
                Some("gif") => "image/gif",
                Some("webp") => "image/webp",
                _ => return,
            };
            let read = cx.background_spawn(async move { std::fs::read(path).ok() }).await;
            this.update(cx, |this, cx| match read {
                Some(bytes) if bytes.len() <= ICON_LIMIT => {
                    let icon = Some((mime, Arc::<[u8]>::from(bytes)));
                    this.net.post(Command::SetGroupIcon { channel, icon });
                }
                Some(_) => {
                    this.connection.set_message(t!("group.icon_too_large"));
                    cx.notify();
                }
                None => {}
            })
            .ok();
        })
        .detach();
    }
}
