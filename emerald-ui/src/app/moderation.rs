//! Moderation from a member's menu: timing out, kicking and banning. Each
//! item shows only when the user's permissions in the server allow it.

use gpui::{AnyElement, Context, prelude::*, rgb};

use super::dialogs::Ask;
use super::place_menu::item;
use super::Emerald;
use crate::theme;
use emerald_discord::Command;
use emerald_discord::model::Snowflake;
use emerald_discord::permissions::{BAN_MEMBERS, KICK_MEMBERS, MODERATE_MEMBERS};

/// The timeouts the menu offers: a label and how long it lasts, in seconds.
const TIMEOUTS: [(&str, u64); 3] = [("10m", 10 * 60), ("1h", 60 * 60), ("1d", 24 * 60 * 60)];

impl Emerald {
    /// The moderation items of `user`'s menu in `guild`, empty when the user
    /// may do none of them. Kick and ban ask first.
    pub(super) fn moderation_items(&self, guild: Snowflake, user: Snowflake, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let granted = self.spaces.current().guild_permissions();
        let mut items = Vec::new();
        if granted & MODERATE_MEMBERS != 0 {
            for (ix, (key, seconds)) in TIMEOUTS.into_iter().enumerate() {
                let label = match key {
                    "10m" => t!("moderation.timeout_10m"),
                    "1h" => t!("moderation.timeout_1h"),
                    _ => t!("moderation.timeout_1d"),
                };
                items.push(
                    item(("user-timeout", ix), label.into(), None)
                        .on_click(cx.listener(move |this, _, _, cx| this.timeout_member(guild, user, Some(seconds), cx)))
                        .into_any_element(),
                );
            }
            items.push(
                item(("user-timeout", TIMEOUTS.len()), t!("moderation.timeout_remove").into(), None)
                    .on_click(cx.listener(move |this, _, _, cx| this.timeout_member(guild, user, None, cx)))
                    .into_any_element(),
            );
        }
        if granted & KICK_MEMBERS != 0 {
            items.push(
                item(("user-kick", 0), t!("moderation.kick").into(), None)
                    .text_color(rgb(theme::feedback_critical()))
                    .on_click(cx.listener(move |this, _, _, cx| this.ask(Ask::Kick { guild, user }, cx)))
                    .into_any_element(),
            );
        }
        if granted & BAN_MEMBERS != 0 {
            items.push(
                item(("user-ban", 0), t!("moderation.ban").into(), None)
                    .text_color(rgb(theme::feedback_critical()))
                    .on_click(cx.listener(move |this, _, _, cx| this.ask(Ask::Ban { guild, user }, cx)))
                    .into_any_element(),
            );
        }
        items
    }

    /// Times `user` out for `seconds` from now, or lifts the timeout.
    fn timeout_member(&mut self, guild: Snowflake, user: Snowflake, seconds: Option<u64>, cx: &mut Context<Self>) {
        self.user_menu = None;
        let until = seconds.map(|seconds| crate::time::iso_utc(crate::time::now_secs() + seconds as i64));
        self.net.post(Command::Timeout { guild, user, until });
        cx.notify();
    }
}
